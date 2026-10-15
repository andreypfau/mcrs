use crate::checks::Outcome;
use crate::parity::Selection;
use crate::scope::{self, Head};
use crate::source::{self, Files, MANIFEST, PackageMap, Tree, Trees, verdict};
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Attribute, ExprMethodCall, FieldValue, Ident, ImplItemFn, ItemFn, Macro, Member, Meta,
    Signature, TraitItemFn,
};

const FIX: &str = "select the test out with a module-path marker and a nextest profile, or gate it on the configuration it needs, and never hide it; a crate's unsafe fn count may not rise";
const IGNORED: &str = "ignored test";
const ALWAYS_FALSE: &str = "cfg predicate is always false";
const CFG_TEST: &str = "cfg!(test)";
const NOT_TEST: &str = "not(test) predicate";
const FLAG: &str = "harness marks a case ignored";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}

enum Pred {
    Name(String),
    Call(String, Vec<Pred>),
    Other,
}

fn split_commas(stream: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut parts = vec![Vec::new()];
    for tree in stream {
        match &tree {
            TokenTree::Punct(punct) if punct.as_char() == ',' => parts.push(Vec::new()),
            _ => parts.last_mut().expect("starts non-empty").push(tree),
        }
    }
    if parts.last().is_some_and(Vec::is_empty) {
        parts.pop();
    }
    parts
}

impl Pred {
    fn parse(item: &[TokenTree]) -> Pred {
        match item {
            [TokenTree::Ident(name)] => Pred::Name(name.to_string()),
            [TokenTree::Ident(name), TokenTree::Group(args)]
                if args.delimiter() == Delimiter::Parenthesis =>
            {
                let children = split_commas(args.stream())
                    .iter()
                    .map(|child| Pred::parse(child))
                    .collect();
                Pred::Call(name.to_string(), children)
            }
            _ => Pred::Other,
        }
    }

    fn truth(&self) -> Truth {
        match self {
            Pred::Name(name) => match name.as_str() {
                "true" => Truth::True,
                "false" => Truth::False,
                _ => Truth::Unknown,
            },
            Pred::Call(name, children) => {
                let values: Vec<Truth> = children.iter().map(Pred::truth).collect();
                let every = |wanted| values.iter().all(|value| *value == wanted);
                let some = |wanted| values.contains(&wanted);
                match name.as_str() {
                    "any" if some(Truth::True) => Truth::True,
                    "any" if every(Truth::False) => Truth::False,
                    "all" if some(Truth::False) => Truth::False,
                    "all" if every(Truth::True) => Truth::True,
                    "not" if values.len() == 1 => match values[0] {
                        Truth::True => Truth::False,
                        Truth::False => Truth::True,
                        Truth::Unknown => Truth::Unknown,
                    },
                    _ => Truth::Unknown,
                }
            }
            Pred::Other => Truth::Unknown,
        }
    }

    fn tests(&self, negated: bool, found: &mut Vec<bool>) {
        match self {
            Pred::Name(name) if name == "test" => found.push(negated),
            Pred::Call(name, children) => {
                for child in children {
                    child.tests(negated || name == "not", found);
                }
            }
            _ => {}
        }
    }
}

struct UnsafeFn {
    package: String,
    path: String,
    line: usize,
    name: String,
}

struct Walk<'a> {
    path: &'a str,
    package: &'a str,
    harness: bool,
    found: &'a mut BTreeSet<String>,
    unsafe_fns: &'a mut Vec<UnsafeFn>,
}

fn unsafe_fn_name(tokens: &[TokenTree], at: usize) -> Option<String> {
    let mut next = at + 1;
    if matches!(tokens.get(next), Some(TokenTree::Ident(word)) if word == "extern") {
        next += 1;
        if matches!(tokens.get(next), Some(TokenTree::Literal(_))) {
            next += 1;
        }
    }
    if !matches!(tokens.get(next), Some(TokenTree::Ident(word)) if word == "fn") {
        return None;
    }
    match (tokens.get(next + 1), tokens.get(next + 2)) {
        (Some(TokenTree::Ident(name)), _) => Some(name.to_string()),
        (Some(TokenTree::Punct(dollar)), Some(TokenTree::Ident(name)))
            if dollar.as_char() == '$' =>
        {
            Some(format!("${name}"))
        }
        _ => None,
    }
}

fn names_ignored_in_tokens(tokens: &[TokenTree], at: usize) -> bool {
    let next = tokens.get(at + 1);
    let field = matches!(next, Some(TokenTree::Punct(colon))
        if colon.as_char() == ':' && colon.spacing() == Spacing::Alone);
    let after_dot =
        at > 0 && matches!(&tokens[at - 1], TokenTree::Punct(dot) if dot.as_char() == '.');
    let call = after_dot
        && matches!(next, Some(TokenTree::Group(args)) if args.delimiter() == Delimiter::Parenthesis);
    field || call
}

impl Walk<'_> {
    fn report(&mut self, line: usize, what: &str) {
        self.found.insert(format!("{}:{line}: {what}", self.path));
    }

    fn predicate(&mut self, item: &[TokenTree], line: usize, expression: bool) {
        let pred = Pred::parse(item);
        if pred.truth() == Truth::False {
            self.report(line, ALWAYS_FALSE);
        }
        let mut uses = Vec::new();
        pred.tests(false, &mut uses);
        for negated in uses {
            if negated {
                self.report(line, NOT_TEST);
            } else if expression {
                self.report(line, CFG_TEST);
            }
        }
    }

    fn cfg_macro(&mut self, tokens: &TokenStream, line: usize) {
        if let Some(item) = split_commas(tokens.clone()).first() {
            self.predicate(item, line, true);
        }
    }

    fn attribute(&mut self, meta: &Meta) {
        let line = source::line(meta.path().span());
        if meta
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "ignore")
        {
            self.report(line, IGNORED);
        }
        let Meta::List(list) = meta else {
            return;
        };
        if list.path.is_ident("cfg") {
            if let Some(item) = split_commas(list.tokens.clone()).first() {
                self.predicate(item, line, false);
            }
        } else if list.path.is_ident("cfg_attr") {
            let mut items = split_commas(list.tokens.clone()).into_iter();
            if let Some(condition) = items.next() {
                self.predicate(&condition, line, false);
            }
            for rest in items {
                if let Ok(nested) = syn::parse2::<Meta>(rest.into_iter().collect()) {
                    self.attribute(&nested);
                }
            }
        }
    }

    fn tokens(&mut self, stream: &TokenStream) {
        source::walk_tokens(stream, &mut |tokens, at| match &tokens[at] {
            TokenTree::Punct(_) => {
                if let Some((meta, _)) = source::attribute_at(tokens, at) {
                    self.attribute(&meta);
                }
            }
            TokenTree::Ident(word) => self.word(word, tokens, at),
            _ => {}
        });
    }

    fn word(&mut self, word: &Ident, tokens: &[TokenTree], at: usize) {
        let line = source::line(word.span());
        if word == "with_ignored_flag" {
            self.report(line, FLAG);
        } else if word == "cfg" {
            if let (Some(TokenTree::Punct(bang)), Some(TokenTree::Group(args))) =
                (tokens.get(at + 1), tokens.get(at + 2))
                && bang.as_char() == '!'
            {
                self.cfg_macro(&args.stream(), line);
            }
        } else if word == "unsafe" {
            if let Some(name) = unsafe_fn_name(tokens, at) {
                self.add_unsafe(line, name);
            }
        } else if word == "ignored" && self.harness && names_ignored_in_tokens(tokens, at) {
            self.report(line, FLAG);
        }
    }

    fn add_unsafe(&mut self, line: usize, name: String) {
        self.unsafe_fns.push(UnsafeFn {
            package: self.package.to_owned(),
            path: self.path.to_owned(),
            line,
            name,
        });
    }

    fn signature(&mut self, sig: &Signature) {
        if sig.unsafety.is_some() {
            self.add_unsafe(source::line(sig.ident.span()), sig.ident.to_string());
        }
    }
}

impl<'ast> Visit<'ast> for Walk<'_> {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.attribute(&attr.meta);
        if let Meta::List(list) = &attr.meta {
            self.tokens(&list.tokens);
        }
        visit::visit_attribute(self, attr);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        if mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "cfg")
        {
            self.cfg_macro(&mac.tokens, source::line(mac.path.span()));
        }
        self.tokens(&mac.tokens);
        visit::visit_macro(self, mac);
    }

    fn visit_ident(&mut self, ident: &'ast Ident) {
        if ident == "with_ignored_flag" {
            self.report(source::line(ident.span()), FLAG);
        }
    }

    fn visit_field_value(&mut self, node: &'ast FieldValue) {
        if self.harness && matches!(&node.member, Member::Named(name) if name == "ignored") {
            self.report(source::line(node.member.span()), FLAG);
        }
        visit::visit_field_value(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if self.harness && node.method == "ignored" {
            self.report(source::line(node.method.span()), FLAG);
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.signature(&node.sig);
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.signature(&node.sig);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast TraitItemFn) {
        self.signature(&node.sig);
        visit::visit_trait_item_fn(self, node);
    }
}

#[derive(Default)]
struct Roles {
    harness: BTreeSet<String>,
    build: BTreeSet<String>,
}

#[derive(Default)]
struct Scan {
    findings: BTreeSet<String>,
    unsafe_fns: Vec<UnsafeFn>,
    unparsed: Vec<String>,
}

fn cfg_name(spec: &str) -> &str {
    let line = spec.lines().next().unwrap_or("").trim();
    let inner = line.strip_prefix("cfg(").unwrap_or(line);
    inner.split([',', ')']).next().unwrap_or("").trim()
}

fn scan_sources(
    files: &Files,
    packages: &PackageMap,
    roles: &Roles,
    declared: &BTreeSet<String>,
) -> Scan {
    let mut scan = Scan::default();
    for (path, text) in files {
        let parsed = match source::parse(path, text) {
            Ok(parsed) => parsed,
            Err(problem) => {
                scan.unparsed.push(problem);
                continue;
            }
        };
        Walk {
            path,
            package: packages.owner(path),
            harness: roles.harness.contains(path),
            found: &mut scan.findings,
            unsafe_fns: &mut scan.unsafe_fns,
        }
        .visit_file(&parsed);
        if !roles.build.contains(path) {
            continue;
        }
        for (literal, line) in source::string_literals(&parsed) {
            for spec in literal.split("rustc-check-cfg=").skip(1) {
                let name = cfg_name(spec);
                if !declared.contains(name) {
                    scan.findings.insert(format!(
                        "{path}:{line}: build script declares cfg {name}, which the workspace unexpected_cfgs list does not"
                    ));
                }
            }
        }
    }
    scan
}

#[derive(Deserialize)]
struct Targets {
    package: Option<BuildKey>,
    #[serde(default)]
    test: Vec<Target>,
    #[serde(default)]
    bench: Vec<Target>,
}

#[derive(Deserialize)]
struct BuildKey {
    build: Option<Build>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Build {
    Path(String),
    Enabled(bool),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Target {
    name: Option<String>,
    path: Option<String>,
    harness: Option<bool>,
}

fn roles(tree: &Tree, files: &Files, scope: Option<&BTreeSet<String>>) -> Result<Roles, String> {
    let mut harness_roots = Vec::new();
    let mut build_roots = Vec::new();
    let present = |candidates: Vec<String>| candidates.into_iter().find(|c| files.contains_key(c));
    for (path, text) in &tree.manifests {
        if !tree.in_scope(path, scope) {
            continue;
        }
        let manifest: Targets =
            toml::from_str(text).map_err(|e| format!("{path} does not parse: {}", e.message()))?;
        let dir = source::parent(path);
        for (folder, targets) in [("tests", &manifest.test), ("benches", &manifest.bench)] {
            for target in targets.iter().filter(|t| t.harness == Some(false)) {
                let candidates = match (&target.path, &target.name) {
                    (Some(given), _) => vec![source::join(dir, given)],
                    (None, Some(name)) => vec![
                        source::join(dir, &format!("{folder}/{name}.rs")),
                        source::join(dir, &format!("{folder}/{name}/main.rs")),
                    ],
                    (None, None) => continue,
                };
                harness_roots.extend(present(candidates));
            }
        }
        let Some(package) = manifest.package else {
            continue;
        };
        let scripts = match package.build {
            None | Some(Build::Enabled(true)) => vec!["build.rs".to_owned()],
            Some(Build::Enabled(false)) => Vec::new(),
            Some(Build::Path(script)) => vec![script],
            Some(Build::Many(scripts)) => scripts,
        };
        build_roots.extend(
            scripts
                .iter()
                .filter_map(|script| present(vec![source::join(dir, script)])),
        );
    }
    let mut roles = Roles::default();
    for (roots, into) in [
        (&harness_roots, &mut roles.harness),
        (&build_roots, &mut roles.build),
    ] {
        for root in roots {
            into.extend(source::module_files(root, files)?);
        }
    }
    Ok(roles)
}

#[derive(Deserialize, Default)]
struct Root {
    workspace: Option<WorkspaceLints>,
}

#[derive(Deserialize)]
struct WorkspaceLints {
    lints: Option<LintTables>,
}

#[derive(Deserialize)]
struct LintTables {
    rust: Option<RustLints>,
}

#[derive(Deserialize)]
struct RustLints {
    unexpected_cfgs: Option<UnexpectedCfgs>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UnexpectedCfgs {
    Table {
        #[serde(rename = "check-cfg", default)]
        check_cfg: Vec<String>,
    },
    Level(Plain),
}

struct Plain;

impl<'de> Deserialize<'de> for Plain {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Plain, D::Error> {
        String::deserialize(deserializer).map(|_| Plain)
    }
}

fn declared_cfgs(tree: &Tree) -> Result<BTreeSet<String>, String> {
    let Some(text) = tree.manifests.get(MANIFEST) else {
        return Ok(BTreeSet::new());
    };
    let root: Root =
        toml::from_str(text).map_err(|e| format!("{MANIFEST} does not parse: {}", e.message()))?;
    let table = root
        .workspace
        .and_then(|workspace| workspace.lints)
        .and_then(|lints| lints.rust)
        .and_then(|rust| rust.unexpected_cfgs);
    Ok(match table {
        Some(UnexpectedCfgs::Table { check_cfg }) => check_cfg
            .iter()
            .map(|spec| cfg_name(spec).to_owned())
            .collect(),
        Some(UnexpectedCfgs::Level(_)) | None => BTreeSet::new(),
    })
}

fn scan_tree(tree: &Tree, scope: Option<&BTreeSet<String>>) -> Result<Scan, String> {
    let files = tree.read_rust(|path| tree.in_scope(path, scope))?;
    let roles = roles(tree, &files, scope)?;
    Ok(scan_sources(
        &files,
        &tree.packages,
        &roles,
        &declared_cfgs(tree)?,
    ))
}

fn unsafe_fns_of(tree: &Tree, packages: &BTreeSet<String>) -> Result<Vec<UnsafeFn>, String> {
    let files = tree.read_rust(|path| packages.contains(tree.packages.owner(path)))?;
    let scan = scan_sources(&files, &tree.packages, &Roles::default(), &BTreeSet::new());
    Ok(scan.unsafe_fns)
}

fn rises(now: &[UnsafeFn], before: &[UnsafeFn]) -> BTreeSet<String> {
    let mut per_package: BTreeMap<&str, Vec<&UnsafeFn>> = BTreeMap::new();
    for function in now {
        per_package
            .entry(function.package.as_str())
            .or_default()
            .push(function);
    }
    let mut over = BTreeSet::new();
    for (package, current) in per_package {
        let earlier: Vec<&UnsafeFn> = before.iter().filter(|f| f.package == package).collect();
        if current.len() <= earlier.len() {
            continue;
        }
        let mut unmatched: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        for function in &earlier {
            *unmatched
                .entry((function.path.as_str(), function.name.as_str()))
                .or_default() += 1;
        }
        let added: Vec<String> = current
            .iter()
            .filter(|function| {
                match unmatched.get_mut(&(function.path.as_str(), function.name.as_str())) {
                    Some(left) if *left > 0 => {
                        *left -= 1;
                        false
                    }
                    _ => true,
                }
            })
            .map(|function| format!("{}:{} {}", function.path, function.line, function.name))
            .collect();
        over.insert(format!(
            "{package}: {} unsafe fn, {} at the base; added {}",
            current.len(),
            earlier.len(),
            added.join(", ")
        ));
    }
    over
}

pub(crate) fn check(repo: &Path, base: &str, head: &Head, selection: Selection) -> Outcome {
    let trees = Trees::read(repo, base, head, &selection, |lints| {
        lints.bypasses.is_some()
    })?;
    let now = scan_tree(&trees.head, trees.scope.as_ref())?;
    if !now.unparsed.is_empty() {
        return Err(now.unparsed.join("; "));
    }
    let holders: BTreeSet<String> = now.unsafe_fns.iter().map(|f| f.package.clone()).collect();
    let before = unsafe_fns_of(&trees.base, &holders)?;
    let mut findings = now.findings;
    findings.extend(rises(&now.unsafe_fns, &before));
    let counts = format!(
        "{} unsafe fn in {} packages",
        now.unsafe_fns.len(),
        holders.len()
    );
    if !trees.in_force {
        return Ok(format!(
            "not in force yet at {}; {} bypasses, {counts}",
            scope::short(base),
            findings.len()
        ));
    }
    verdict(&findings, FIX, || {
        format!("no hidden test or always-false cfg; {counts}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::judge_rows_in_parallel;
    use crate::source::fixture::{self, At, Row, Verdict, del, files, put, row};
    use std::collections::BTreeMap;

    fn scan_one(source: &str) -> Vec<String> {
        let tree = files(&[("lib.rs", source)]);
        let packages = PackageMap::new(&files(&[("Cargo.toml", "[package]\nname = \"p\"\n")]))
            .expect("a manifest");
        scan_sources(&tree, &packages, &Roles::default(), &BTreeSet::new())
            .findings
            .into_iter()
            .collect()
    }

    type Case = (&'static str, &'static str, &'static [(usize, &'static str)]);

    fn ignore_cases() -> Vec<Case> {
        vec![
            ("bare ignore", "#[ignore]\nfn f() {}\n", &[(1, IGNORED)]),
            (
                "ignore with a reason",
                "#[ignore = \"slow\"]\nfn f() {}\n",
                &[(1, IGNORED)],
            ),
            (
                "ignore behind cfg_attr",
                "#[cfg_attr(unix, ignore)]\nfn f() {}\n",
                &[(1, IGNORED)],
            ),
            (
                "ignore behind a configuration that is not test",
                "#[cfg_attr(not(debug_assertions), ignore)]\nfn f() {}\n",
                &[(1, IGNORED)],
            ),
            (
                "ignore behind nested cfg_attr",
                "#[cfg_attr(unix, cfg_attr(test, ignore))]\nfn f() {}\n",
                &[(1, IGNORED)],
            ),
            (
                "ignore in a macro_rules body",
                "macro_rules! t { () => { #[ignore] #[test] fn f() {} } }\n",
                &[(1, IGNORED)],
            ),
            (
                "ignore on a later line",
                "fn f() {}\n#[ignore]\nfn g() {}\n",
                &[(2, IGNORED)],
            ),
        ]
    }

    fn predicate_cases() -> Vec<Case> {
        vec![
            (
                "cfg any of nothing",
                "#[cfg(any())]\nfn f() {}\n",
                &[(1, ALWAYS_FALSE)],
            ),
            (
                "cfg false",
                "#[cfg(false)]\nfn f() {}\n",
                &[(1, ALWAYS_FALSE)],
            ),
            (
                "cfg not all of nothing",
                "#[cfg(not(all()))]\nfn f() {}\n",
                &[(1, ALWAYS_FALSE)],
            ),
            (
                "cfg not true",
                "#[cfg(not(true))]\nfn f() {}\n",
                &[(1, ALWAYS_FALSE)],
            ),
            (
                "cfg all holding an empty any",
                "#[cfg(all(unix, any()))]\nfn f() {}\n",
                &[(1, ALWAYS_FALSE)],
            ),
            (
                "cfg not any of two false children",
                "#[cfg(not(any(false, any())))]\nfn f() {}\n",
                &[],
            ),
            (
                "cfg_attr with a predicate that is never true",
                "#[cfg_attr(any(), derive(Debug))]\nfn f() {}\n",
                &[(1, ALWAYS_FALSE)],
            ),
            (
                "cfg! with a predicate that is never true",
                "fn f() {\n    if cfg!(any()) {}\n}\n",
                &[(2, ALWAYS_FALSE)],
            ),
            (
                "cfg!(test)",
                "fn f() {\n    if cfg!(test) {}\n}\n",
                &[(2, CFG_TEST)],
            ),
            (
                "cfg!(test) inside another macro",
                "fn f() {\n    assert!(cfg!(test));\n}\n",
                &[(2, CFG_TEST)],
            ),
            (
                "not(test) on an item",
                "#[cfg(not(test))]\nfn f() {}\n",
                &[(1, NOT_TEST)],
            ),
            (
                "not(test) in cfg_attr",
                "#[cfg_attr(not(test), derive(Debug))]\nstruct S;\n",
                &[(1, NOT_TEST)],
            ),
            (
                "not(test) deep in cfg!",
                "fn f() {\n    let _ = cfg!(all(unix, not(test)));\n}\n",
                &[(2, NOT_TEST)],
            ),
            (
                "the ignored flag of a harness trial",
                "fn f(trial: Trial) {\n    trial.with_ignored_flag(true);\n}\n",
                &[(2, FLAG)],
            ),
            (
                "the ignored flag as a path",
                "fn f() {\n    libtest_mimic::Trial::with_ignored_flag;\n}\n",
                &[(2, FLAG)],
            ),
            (
                "the ignored flag inside a macro",
                "fn f() {\n    run!(trial.with_ignored_flag(true));\n}\n",
                &[(2, FLAG)],
            ),
        ]
    }

    fn other_cases() -> Vec<Case> {
        vec![
            ("cfg test", "#[cfg(test)]\nmod tests {}\n", &[]),
            (
                "cfg_attr test",
                "#[cfg_attr(test, derive(Debug))]\nstruct S;\n",
                &[],
            ),
            (
                "cfg any with a known false and an unknown child",
                "#[cfg(any(unix, any()))]\nfn f() {}\n",
                &[],
            ),
            (
                "cfg debug_assertions",
                "#[cfg(debug_assertions)]\nfn f() {}\n",
                &[],
            ),
            ("cfg all of nothing", "#[cfg(all())]\nfn f() {}\n", &[]),
            (
                "cfg any holding test",
                "#[cfg(any(test, unix))]\nfn f() {}\n",
                &[],
            ),
            (
                "a feature and a target value",
                "#[cfg(all(feature = \"x\", target_os = \"linux\"))]\nfn f() {}\n",
                &[],
            ),
            (
                "ignore as a field of another attribute",
                "#[serde(ignore)]\nstruct S;\n",
                &[],
            ),
            (
                "cfg! over a plain predicate",
                "fn f() -> bool {\n    cfg!(unix)\n}\n",
                &[],
            ),
            (
                "an ignored field outside a harness target",
                "fn f() -> Case {\n    Case { ignored: true }\n}\n",
                &[],
            ),
            (
                "ignore behind a predicate that is never true",
                "#[cfg_attr(any(), ignore)]\nfn f() {}\n",
                &[(1, IGNORED), (1, ALWAYS_FALSE)],
            ),
            (
                "an inner cfg that is never true",
                "#![cfg(any())]\nfn f() {}\n",
                &[(1, ALWAYS_FALSE)],
            ),
            (
                "not(test) beneath an inner not",
                "#[cfg(not(not(test)))]\nfn f() {}\n",
                &[(1, NOT_TEST)],
            ),
        ]
    }

    #[test]
    fn every_way_to_hide_a_test_is_a_bypass() {
        let rows = [ignore_cases(), predicate_cases(), other_cases()].concat();
        for (name, source, expected) in rows {
            let mut wanted: Vec<String> = expected
                .iter()
                .map(|(line, what)| format!("lib.rs:{line}: {what}"))
                .collect();
            wanted.sort();
            assert_eq!(scan_one(source), wanted, "{name}");
        }
    }

    const ROOT: &str = "[package]\nname = \"root\"\nversion = \"0.1.0\"\n\n[lints]\nworkspace = true\n\n[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(getrandom_backend, values(\"wasm_js\"))', 'cfg(web_sys_unstable_apis)'] }\n";
    const OPTED_IN: &str = "[lints]\nworkspace = true\n";
    const LINTS_ON: &str = "[bypasses]\n";
    const LINTS_FILE: &str = "quality/lints.toml";
    const A_MANIFEST: &str = "crates/a/Cargo.toml";
    const A_ROOT: &str = "crates/a/src/lib.rs";
    const B_ROOT: &str = "crates/b/src/lib.rs";
    const B_MANIFEST: &str = "crates/b/Cargo.toml";
    const LISTING: &str = "crates/a/tests/listing.rs";
    const PLAIN: &str = "fn a() {}\n";
    const ONE_UNSAFE: &str = "unsafe fn one() {}\n";
    const TWO_UNSAFE: &str = "unsafe fn one() {}\nunsafe fn two() {}\n";
    const AN_IGNORE: &str = "#[ignore]\nfn one() {}\n";
    const HARNESS_LISTING: &str = "[[test]]\nname = \"listing\"\nharness = false\n";
    const ITEM_WITH_FIELD: &str =
        "struct Case { ignored: bool }\nfn cases() -> Case {\n    Case { ignored: true }\n}\n";
    const NEVER_DECLARED: &str =
        "fn main() {\n    println!(\"cargo::rustc-check-cfg=cfg(never)\");\n}\n";

    fn manifest(name: &str, tables: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n{OPTED_IN}\n{tables}")
    }

    fn standard() -> BTreeMap<&'static str, String> {
        BTreeMap::from([
            ("Cargo.toml", ROOT.to_owned()),
            ("src/main.rs", "fn main() {}\n".to_owned()),
            (A_MANIFEST, manifest("a", "")),
            (A_ROOT, PLAIN.to_owned()),
            (B_MANIFEST, manifest("b", "")),
            (B_ROOT, "fn b() {}\n".to_owned()),
            (LINTS_FILE, LINTS_ON.to_owned()),
        ])
    }

    fn judge(row: &Row) -> Option<String> {
        fixture::judge("bypasses", &standard(), check, row)
    }

    fn unsafe_rows() -> Vec<Row> {
        use Verdict::{Fail, Pass};
        let trait_only = "trait T {\n    unsafe fn m(&self);\n}\nstruct S;\n";
        let with_impl = "trait T {\n    unsafe fn m(&self);\n}\nstruct S;\nimpl T for S {\n    unsafe fn m(&self) {}\n}\n";
        vec![
            row(
                "a new unsafe fn fails and names the function",
                vec![put(A_ROOT, ONE_UNSAFE)],
                vec![put(A_ROOT, TWO_UNSAFE)],
                Fail(&["a: 2 unsafe fn, 1 at the base", "crates/a/src/lib.rs:2 two"]),
            ),
            row(
                "a removed unsafe fn passes",
                vec![put(A_ROOT, TWO_UNSAFE)],
                vec![put(A_ROOT, ONE_UNSAFE)],
                Pass(&["no hidden test or always-false cfg; 1 unsafe fn in 1 packages"]),
            ),
            row(
                "an unsafe trait impl method fails",
                vec![put(A_ROOT, trait_only)],
                vec![put(A_ROOT, with_impl)],
                Fail(&["a: 2 unsafe fn, 1 at the base", "crates/a/src/lib.rs:6 m"]),
            ),
            row(
                "an unsafe fn in a new package fails",
                vec![put(A_ROOT, ONE_UNSAFE)],
                vec![
                    put("crates/c/Cargo.toml", &manifest("c", "")),
                    put("crates/c/src/lib.rs", ONE_UNSAFE),
                ],
                Fail(&["c: 1 unsafe fn, 0 at the base", "crates/c/src/lib.rs:1 one"]),
            ),
            row(
                "an unsafe fn moved to another file of its package passes",
                vec![put(A_ROOT, ONE_UNSAFE)],
                vec![
                    put(A_ROOT, "mod other;\n"),
                    put("crates/a/src/other.rs", ONE_UNSAFE),
                ],
                Pass(&["1 unsafe fn in 1 packages"]),
            ),
            row(
                "an unsafe fn written inside a macro body fails",
                vec![],
                vec![put(
                    A_ROOT,
                    "macro_rules! m {\n    () => {\n        unsafe fn hidden() {}\n    };\n}\n",
                )],
                Fail(&[
                    "a: 1 unsafe fn, 0 at the base",
                    "crates/a/src/lib.rs:3 hidden",
                ]),
            ),
            row(
                "a function pointer type is not an unsafe fn item",
                vec![],
                vec![put(A_ROOT, "type P = unsafe fn(u8);\n")],
                Pass(&["0 unsafe fn in 0 packages"]),
            ),
        ]
    }

    fn harness_rows() -> Vec<Row> {
        use Verdict::{Fail, Pass};
        let listing = |text: &'static str| {
            vec![
                put(A_MANIFEST, &manifest("a", HARNESS_LISTING)),
                put(LISTING, text),
            ]
        };
        vec![
            row(
                "a field initialiser in a custom harness target fails",
                vec![],
                listing(ITEM_WITH_FIELD),
                Fail(&["crates/a/tests/listing.rs:3: harness marks a case ignored"]),
            ),
            row(
                "a shorthand field initialiser fails",
                vec![],
                listing("fn f(ignored: bool) -> Case {\n    Case { ignored }\n}\n"),
                Fail(&["crates/a/tests/listing.rs:2: harness marks a case ignored"]),
            ),
            row(
                "a method call in a custom harness target fails",
                vec![],
                listing("fn f(case: Case) {\n    case.ignored();\n}\n"),
                Fail(&["crates/a/tests/listing.rs:2: harness marks a case ignored"]),
            ),
            row(
                "a field initialiser in a macro of a custom harness target fails",
                vec![],
                listing("fn f() {\n    let _ = vec![Case { ignored: true }];\n}\n"),
                Fail(&["crates/a/tests/listing.rs:2: harness marks a case ignored"]),
            ),
            row(
                "a module of a custom harness target is searched",
                vec![],
                vec![
                    put(A_MANIFEST, &manifest("a", HARNESS_LISTING)),
                    put(LISTING, "mod cases;\n"),
                    put("crates/a/tests/cases.rs", ITEM_WITH_FIELD),
                ],
                Fail(&["crates/a/tests/cases.rs:3: harness marks a case ignored"]),
            ),
            row(
                "a bench target with its own path is searched",
                vec![],
                vec![
                    put(
                        A_MANIFEST,
                        &manifest(
                            "a",
                            "[[bench]]\nname = \"b\"\npath = \"benches/main.rs\"\nharness = false\n",
                        ),
                    ),
                    put("crates/a/benches/main.rs", ITEM_WITH_FIELD),
                ],
                Fail(&["crates/a/benches/main.rs:3: harness marks a case ignored"]),
            ),
            row(
                "the same field in an ordinary test target passes",
                vec![],
                vec![
                    put(A_MANIFEST, &manifest("a", "[[test]]\nname = \"listing\"\n")),
                    put(LISTING, ITEM_WITH_FIELD),
                ],
                Pass(&["no hidden test or always-false cfg"]),
            ),
            row(
                "the same field in a target the manifest does not list passes",
                vec![],
                vec![put(LISTING, ITEM_WITH_FIELD)],
                Pass(&["no hidden test or always-false cfg"]),
            ),
        ]
    }

    fn build_script_rows() -> Vec<Row> {
        use Verdict::{Fail, Pass};
        let build = |text: &'static str| vec![put("crates/a/build.rs", text)];
        vec![
            row(
                "a build script declaring an unknown cfg fails",
                vec![],
                build(NEVER_DECLARED),
                Fail(&["crates/a/build.rs:2: build script declares cfg never"]),
            ),
            row(
                "a build script declaring a cfg the root table declares passes",
                vec![],
                build(
                    "fn main() {\n    println!(\"cargo::rustc-check-cfg=cfg(web_sys_unstable_apis)\");\n}\n",
                ),
                Pass(&["no hidden test or always-false cfg"]),
            ),
            row(
                "the single colon form with values is read the same way",
                vec![],
                build(
                    "fn main() {\n    println!(\"cargo:rustc-check-cfg=cfg(getrandom_backend, values(\\\"wasm_js\\\"))\");\n}\n",
                ),
                Pass(&["no hidden test or always-false cfg"]),
            ),
            row(
                "a declaration built from a format argument fails",
                vec![],
                build(
                    "fn main() {\n    println!(\"cargo::rustc-check-cfg=cfg({})\", \"never\");\n}\n",
                ),
                Fail(&["crates/a/build.rs:2: build script declares cfg {}"]),
            ),
            row(
                "a build script named by the manifest is read",
                vec![],
                vec![
                    put(
                        A_MANIFEST,
                        "[package]\nname = \"a\"\nversion = \"0.1.0\"\nbuild = \"tools/setup.rs\"\n\n[lints]\nworkspace = true\n",
                    ),
                    put("crates/a/tools/setup.rs", NEVER_DECLARED),
                ],
                Fail(&["crates/a/tools/setup.rs:2: build script declares cfg never"]),
            ),
            row(
                "a source file that is not the build script is left to the string rules",
                vec![],
                vec![put("crates/a/src/build.rs", NEVER_DECLARED)],
                Pass(&["no hidden test or always-false cfg"]),
            ),
        ]
    }

    fn tree_rows() -> Vec<Row> {
        use Verdict::{Fail, Pass};
        vec![
            row(
                "an ignore in a committed file fails",
                vec![],
                vec![
                    put("crates/a/src/extra.rs", AN_IGNORE),
                    put(A_ROOT, "mod extra;\n"),
                ],
                Fail(&["crates/a/src/extra.rs:1: ignored test"]),
            ),
            row(
                "an ignore in an untracked file passes for the working tree",
                vec![],
                vec![put("crates/a/src/extra.rs", AN_IGNORE)],
                Pass(&["no hidden test or always-false cfg"]),
            )
            .at(At::Worktree),
            row(
                "an ignore in a modified tracked file fails for the working tree",
                vec![],
                vec![put(A_ROOT, AN_IGNORE)],
                Fail(&["crates/a/src/lib.rs:1: ignored test"]),
            )
            .at(At::Worktree),
            row(
                "an ignore in the head commit fails while the checkout sits at the base",
                vec![],
                vec![put(A_ROOT, AN_IGNORE)],
                Fail(&["crates/a/src/lib.rs:1: ignored test"]),
            )
            .at(At::DetachedAtBase),
            row(
                "an always-false cfg in the head fails",
                vec![],
                vec![put(A_ROOT, "#[cfg(any())]\nfn gone() {}\n")],
                Fail(&["crates/a/src/lib.rs:1: cfg predicate is always false"]),
            ),
            row(
                "a file that does not parse fails naming its path",
                vec![],
                vec![put(A_ROOT, "fn broken( {\n")],
                Fail(&["crates/a/src/lib.rs:", "does not parse"]),
            ),
            row(
                "a clean tree reports its counts",
                vec![],
                vec![],
                Pass(&["no hidden test or always-false cfg; 0 unsafe fn in 0 packages"]),
            ),
            row(
                "without a lints file the check is not in force and counts the findings",
                vec![
                    del(LINTS_FILE),
                    put(A_ROOT, "#[ignore]\nfn one() {}\nunsafe fn two() {}\n"),
                ],
                vec![],
                Pass(&[
                    "not in force yet at ",
                    "; 1 bypasses, 1 unsafe fn in 1 packages",
                ]),
            ),
            row(
                "a lints file without the bypasses table is not in force",
                vec![put(LINTS_FILE, "[suppressions]\n"), put(A_ROOT, AN_IGNORE)],
                vec![],
                Pass(&[
                    "not in force yet at ",
                    "; 1 bypasses, 0 unsafe fn in 0 packages",
                ]),
            ),
            row(
                "not in force, a rising unsafe count is reported and passes",
                vec![del(LINTS_FILE), put(A_ROOT, ONE_UNSAFE)],
                vec![put(A_ROOT, TWO_UNSAFE)],
                Pass(&["; 1 bypasses, 2 unsafe fn in 1 packages"]),
            ),
            row(
                "a package outside the changed paths may hold an ignore for the edit trigger",
                vec![],
                vec![put(A_ROOT, AN_IGNORE), put(B_ROOT, "fn b2() {}\n")],
                Pass(&["no hidden test or always-false cfg"]),
            )
            .changed(&[B_ROOT]),
            row(
                "the changed package is judged for the edit trigger",
                vec![],
                vec![put(A_ROOT, AN_IGNORE), put(B_ROOT, "fn b2() {}\n")],
                Fail(&["crates/a/src/lib.rs:1: ignored test"]),
            )
            .changed(&[A_ROOT]),
        ]
    }

    #[test]
    fn the_check_judges_bypasses_against_the_base() {
        let mut rows = unsafe_rows();
        rows.extend(harness_rows());
        rows.extend(build_script_rows());
        rows.extend(tree_rows());
        judge_rows_in_parallel(&rows, judge);
    }
}
