use crate::gate::{Lints, lints_at};
use crate::git;
use crate::parity::Selection;
use crate::scope::{self, Head};
use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{AttrStyle, Attribute, Expr, ExprLit, Item, Lit, LitStr, Macro, Meta, Token};

pub type Files = BTreeMap<String, String>;

pub fn rust_files(repo: &Path, head: &Head) -> Result<Vec<String>, String> {
    let mut paths = match head {
        Head::Worktree => {
            let listing = git::run(
                repo,
                &[
                    "ls-files",
                    "-z",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                ],
            )
            .map_err(|e| e.to_string())?;
            listing
                .split(|byte| *byte == 0)
                .filter(|path| !path.is_empty())
                .map(|path| String::from_utf8_lossy(path).into_owned())
                .collect()
        }
        Head::Commit(_) => scope::tracked(repo, head)?,
    };
    paths.retain(|path| path.ends_with(".rs"));
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub fn read_files(repo: &Path, head: &Head, paths: &[String]) -> Result<Files, String> {
    let mut files = Files::new();
    match head {
        Head::Worktree => {
            for path in paths {
                match std::fs::read(repo.join(path)) {
                    Ok(bytes) => {
                        files.insert(path.clone(), String::from_utf8_lossy(&bytes).into_owned());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("could not read {path}: {e}")),
                }
            }
        }
        Head::Commit(sha) => {
            let specs: Vec<String> = paths.iter().map(|path| format!("{sha}:{path}")).collect();
            for (path, blob) in paths.iter().zip(scope::blobs(repo, &specs)?) {
                let bytes = blob.ok_or_else(|| format!("{path} is not in {sha}"))?;
                files.insert(path.clone(), String::from_utf8_lossy(&bytes).into_owned());
            }
        }
    }
    Ok(files)
}

pub fn parse(path: &str, text: &str) -> Result<syn::File, String> {
    syn::parse_file(text).map_err(|e| {
        let at = e.span().start();
        format!("{path}:{}: does not parse: {e}", at.line)
    })
}

pub fn line(span: Span) -> usize {
    span.start().line
}

struct StringLiterals(Vec<(String, usize)>);

impl<'ast> Visit<'ast> for StringLiterals {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        if !attr.path().is_ident("doc") {
            visit::visit_attribute(self, attr);
        }
    }

    fn visit_lit_str(&mut self, lit: &'ast LitStr) {
        self.0.push((lit.value(), line(lit.span())));
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        walk_tokens(&mac.tokens, &mut |tokens, at| {
            let TokenTree::Literal(literal) = &tokens[at] else {
                return;
            };
            let after_doc = at >= 2
                && matches!(&tokens[at - 1], TokenTree::Punct(p) if p.as_char() == '=')
                && matches!(&tokens[at - 2], TokenTree::Ident(i) if i == "doc");
            if let (false, Some(text)) = (after_doc, string_literal(literal)) {
                self.0.push((text, line(literal.span())));
            }
        });
    }
}

pub fn string_literals(file: &syn::File) -> Vec<(String, usize)> {
    let mut found = StringLiterals(Vec::new());
    found.visit_file(file);
    found.0
}

pub fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|s| s.ident == "test")
    })
}

pub fn has_cfg(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
}

pub fn walk_tokens(stream: &TokenStream, visit: &mut dyn FnMut(&[TokenTree], usize)) {
    let tokens: Vec<TokenTree> = stream.clone().into_iter().collect();
    for index in 0..tokens.len() {
        visit(&tokens, index);
        if let TokenTree::Group(group) = &tokens[index] {
            walk_tokens(&group.stream(), visit);
        }
    }
}

pub fn string_literal(literal: &proc_macro2::Literal) -> Option<String> {
    match Lit::new(literal.clone()) {
        Lit::Str(text) => Some(text.value()),
        _ => None,
    }
}

pub fn doc_line(attr: &Attribute) -> Option<String> {
    let Meta::NameValue(pair) = &attr.meta else {
        return None;
    };
    if !pair.path.is_ident("doc") {
        return None;
    }
    match &pair.value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) => Some(text.value()),
        _ => None,
    }
}

pub struct ModuleView<'a> {
    pub file: &'a str,
    pub module: &'a [String],
    pub attrs: &'a [Attribute],
    pub items: &'a [Item],
}

struct Walk<'a> {
    files: &'a Files,
    seen: BTreeSet<String>,
}

pub fn walk_modules(
    root: &str,
    files: &Files,
    visit: &mut dyn FnMut(&ModuleView),
) -> Result<(), String> {
    let mut walk = Walk {
        files,
        seen: BTreeSet::new(),
    };
    walk.file(root, &[], true, visit)
}

pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

pub fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

pub fn join(base: &str, rest: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn path_attribute(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        let Meta::NameValue(pair) = &attr.meta else {
            return None;
        };
        if !pair.path.is_ident("path") {
            return None;
        }
        match &pair.value {
            Expr::Lit(ExprLit {
                lit: Lit::Str(text),
                ..
            }) => Some(text.value()),
            _ => None,
        }
    })
}

impl Walk<'_> {
    fn file(
        &mut self,
        path: &str,
        module: &[String],
        mod_rs: bool,
        visit: &mut dyn FnMut(&ModuleView),
    ) -> Result<(), String> {
        if !self.seen.insert(path.to_owned()) {
            return Ok(());
        }
        let text = self
            .files
            .get(path)
            .ok_or_else(|| format!("{path} is not among the tracked Rust files"))?;
        let parsed = parse(path, text)?;
        let owner = if mod_rs {
            parent(path).to_owned()
        } else {
            let stem = file_name(path).trim_end_matches(".rs");
            join(parent(path), stem)
        };
        self.items(
            path,
            module,
            &parsed.attrs,
            &parsed.items,
            &owner,
            &[],
            visit,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the file, module path, attributes and items of one module plus the owner, the inline path and the visitor it reports to"
    )]
    fn items(
        &mut self,
        path: &str,
        module: &[String],
        attrs: &[Attribute],
        items: &[Item],
        owner: &str,
        inline: &[String],
        visit: &mut dyn FnMut(&ModuleView),
    ) -> Result<(), String> {
        visit(&ModuleView {
            file: path,
            module,
            attrs,
            items,
        });
        let children = join(owner, &inline.join("/"));
        for item in items {
            let Item::Mod(declared) = item else {
                continue;
            };
            let name = declared.ident.to_string();
            let mut below = module.to_vec();
            below.push(name.clone());
            if let Some((_, content)) = &declared.content {
                let mut nested = inline.to_vec();
                nested.push(name);
                self.items(path, &below, &[], content, owner, &nested, visit)?;
                continue;
            }
            let (candidates, via_path) = match path_attribute(&declared.attrs) {
                Some(given) if inline.is_empty() => (vec![join(parent(path), &given)], true),
                Some(given) => (vec![join(&children, &given)], true),
                None => (
                    vec![
                        join(&children, &format!("{name}.rs")),
                        join(&children, &format!("{name}/mod.rs")),
                    ],
                    false,
                ),
            };
            match candidates.iter().find(|c| self.files.contains_key(*c)) {
                Some(found) => {
                    let mod_rs = via_path || file_name(found) == "mod.rs";
                    self.file(found, &below, mod_rs, visit)?;
                }
                None if has_cfg(&declared.attrs) => {}
                None => {
                    return Err(format!(
                        "{path}: `mod {name};` has no file; looked for {}",
                        candidates.join(" and ")
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Allow,
    Expect,
    Warn,
    Deny,
    Forbid,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LintAttr {
    pub level: Level,
    pub inner: bool,
    pub lints: Vec<String>,
    pub reason: Option<String>,
    pub line: usize,
}

impl Level {
    fn of(path: &syn::Path) -> Option<Level> {
        let name = path.get_ident()?.to_string();
        Some(match name.as_str() {
            "allow" => Level::Allow,
            "expect" => Level::Expect,
            "warn" => Level::Warn,
            "deny" => Level::Deny,
            "forbid" => Level::Forbid,
            _ => return None,
        })
    }
}

type Metas = Punctuated<Meta, Token![,]>;

fn path_text(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn lint_attr(meta: &Meta, inner: bool, found: &mut Vec<LintAttr>) {
    let Meta::List(list) = meta else {
        return;
    };
    if list.path.is_ident("cfg_attr") {
        if let Ok(args) = list.parse_args_with(Metas::parse_terminated) {
            for nested in args.iter().skip(1) {
                lint_attr(nested, inner, found);
            }
        }
        return;
    }
    let Some(level) = Level::of(&list.path) else {
        return;
    };
    let mut lints = Vec::new();
    let mut reason = None;
    match list.parse_args_with(Metas::parse_terminated) {
        Ok(args) => {
            for arg in &args {
                match arg {
                    Meta::NameValue(pair) if pair.path.is_ident("reason") => {
                        reason = Some(match &pair.value {
                            Expr::Lit(ExprLit {
                                lit: Lit::Str(text),
                                ..
                            }) => text.value(),
                            _ => String::new(),
                        });
                    }
                    other => lints.push(path_text(other.path())),
                }
            }
        }
        Err(_) => lints.push(list.tokens.to_string()),
    }
    found.push(LintAttr {
        level,
        inner,
        lints,
        reason,
        line: line(list.path.span()),
    });
}

pub fn attribute_at(tokens: &[TokenTree], at: usize) -> Option<(Meta, bool)> {
    let TokenTree::Punct(pound) = &tokens[at] else {
        return None;
    };
    if pound.as_char() != '#' {
        return None;
    }
    let inner = matches!(tokens.get(at + 1), Some(TokenTree::Punct(bang)) if bang.as_char() == '!');
    let Some(TokenTree::Group(group)) = tokens.get(at + 1 + usize::from(inner)) else {
        return None;
    };
    if group.delimiter() != Delimiter::Bracket {
        return None;
    }
    syn::parse2::<Meta>(group.stream())
        .ok()
        .map(|meta| (meta, inner))
}

fn lint_attrs_in_tokens(stream: &TokenStream, found: &mut Vec<LintAttr>) {
    walk_tokens(stream, &mut |tokens, at| {
        if let Some((meta, inner)) = attribute_at(tokens, at) {
            lint_attr(&meta, inner, found);
        }
    });
}

struct LintWalk(Vec<LintAttr>);

impl<'ast> Visit<'ast> for LintWalk {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        lint_attr(
            &attr.meta,
            matches!(attr.style, AttrStyle::Inner(_)),
            &mut self.0,
        );
        if let Meta::List(list) = &attr.meta {
            lint_attrs_in_tokens(&list.tokens, &mut self.0);
        }
        visit::visit_attribute(self, attr);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        lint_attrs_in_tokens(&mac.tokens, &mut self.0);
    }
}

pub fn lint_attributes(file: &syn::File) -> Vec<LintAttr> {
    let mut walk = LintWalk(Vec::new());
    walk.visit_file(file);
    walk.0
}

#[derive(Deserialize)]
struct ManifestName {
    package: Option<PackageName>,
}

#[derive(Deserialize)]
struct PackageName {
    name: String,
}

pub struct PackageMap(Vec<(String, String)>);

impl PackageMap {
    pub fn new(manifests: &Files) -> Result<PackageMap, String> {
        let mut owners = Vec::new();
        for (path, text) in manifests {
            let manifest: ManifestName = toml::from_str(text)
                .map_err(|e| format!("{path} does not parse: {}", e.message()))?;
            if let Some(package) = manifest.package {
                owners.push((parent(path).to_owned(), package.name));
            }
        }
        owners.sort_by_key(|(dir, _)| std::cmp::Reverse(dir.len()));
        Ok(PackageMap(owners))
    }

    pub fn contains(&self, package: &str) -> bool {
        self.0.iter().any(|(_, name)| name == package)
    }

    pub fn owner(&self, path: &str) -> &str {
        self.0
            .iter()
            .find(|(dir, _)| {
                dir.is_empty()
                    || path
                        .strip_prefix(dir.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .map_or("", |(_, name)| name)
    }
}

pub(crate) const MANIFEST: &str = "Cargo.toml";

pub(crate) struct Tree<'a> {
    pub(crate) repo: &'a Path,
    pub(crate) head: Head,
    pub(crate) paths: Vec<String>,
    pub(crate) manifests: Files,
    pub(crate) packages: PackageMap,
}

impl<'a> Tree<'a> {
    pub(crate) fn read(repo: &'a Path, head: Head) -> Result<Tree<'a>, String> {
        let paths = scope::tracked(repo, &head)?;
        let manifest_paths: Vec<String> = paths
            .iter()
            .filter(|path| file_name(path) == MANIFEST)
            .cloned()
            .collect();
        let manifests = read_files(repo, &head, &manifest_paths)?;
        let packages = PackageMap::new(&manifests)?;
        Ok(Tree {
            repo,
            head,
            paths,
            manifests,
            packages,
        })
    }

    pub(crate) fn in_scope(&self, path: &str, scope: Option<&BTreeSet<String>>) -> bool {
        scope.is_none_or(|owners| owners.contains(self.packages.owner(path)))
    }

    pub(crate) fn read_rust(&self, keep: impl Fn(&str) -> bool) -> Result<Files, String> {
        let rust: Vec<String> = self
            .paths
            .iter()
            .filter(|path| path.ends_with(".rs") && keep(path))
            .cloned()
            .collect();
        read_files(self.repo, &self.head, &rust)
    }
}

pub(crate) struct Trees<'a> {
    pub(crate) in_force: bool,
    pub(crate) head: Tree<'a>,
    pub(crate) base: Tree<'a>,
    pub(crate) scope: Option<BTreeSet<String>>,
}

impl<'a> Trees<'a> {
    pub(crate) fn read(
        repo: &'a Path,
        base: &str,
        head: &Head,
        selection: &Selection,
        in_force: impl Fn(&Lints) -> bool,
    ) -> Result<Trees<'a>, String> {
        let in_force = lints_at(repo, base, head)?.is_some_and(|lints| in_force(&lints));
        let head = Tree::read(repo, head.clone())?;
        let base = Tree::read(repo, Head::Commit(base.to_owned()))?;
        let scope = scoped(selection, &head, &base);
        Ok(Trees {
            in_force,
            head,
            base,
            scope,
        })
    }
}

const SHOWN: usize = 25;

pub(crate) fn failure<'a>(findings: impl IntoIterator<Item = &'a String>, fix: &str) -> String {
    let findings: Vec<&String> = findings.into_iter().collect();
    let shown: Vec<&str> = findings
        .iter()
        .take(SHOWN)
        .map(|line| line.as_str())
        .collect();
    let more = findings.len() - shown.len();
    let tail = if more > 0 {
        format!("; and {more} more")
    } else {
        String::new()
    };
    format!("{}{tail}; {fix}", shown.join("; "))
}

pub(crate) fn verdict(
    findings: &BTreeSet<String>,
    fix: &str,
    clean: impl FnOnce() -> String,
) -> Result<String, String> {
    if findings.is_empty() {
        Ok(clean())
    } else {
        Err(failure(findings, fix))
    }
}

pub(crate) fn scoped(selection: &Selection, head: &Tree, base: &Tree) -> Option<BTreeSet<String>> {
    let Selection::Changed(paths) = selection else {
        return None;
    };
    Some(
        paths
            .iter()
            .flat_map(|path| [head.packages.owner(path), base.packages.owner(path)])
            .map(str::to_owned)
            .collect(),
    )
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;
    use crate::scope::tests::Repo;

    pub(crate) type Edit = (&'static str, Option<String>);

    pub(crate) fn files(rows: &[(&str, &str)]) -> Files {
        rows.iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    pub(crate) fn put(path: &'static str, text: &str) -> Edit {
        (path, Some(text.to_owned()))
    }

    pub(crate) fn del(path: &'static str) -> Edit {
        (path, None)
    }

    #[derive(Clone, Copy)]
    pub(crate) enum At {
        Commit,
        Worktree,
        DetachedAtBase,
    }

    pub(crate) enum Verdict {
        Pass(&'static [&'static str]),
        Fail(&'static [&'static str]),
    }

    pub(crate) struct Row {
        name: String,
        base: Vec<Edit>,
        head: Vec<Edit>,
        at: At,
        changed: Option<Vec<String>>,
        verdict: Verdict,
    }

    pub(crate) fn row(name: &str, base: Vec<Edit>, head: Vec<Edit>, verdict: Verdict) -> Row {
        Row {
            name: name.to_owned(),
            base,
            head,
            at: At::Commit,
            changed: None,
            verdict,
        }
    }

    impl Row {
        pub(crate) fn at(mut self, at: At) -> Row {
            self.at = at;
            self
        }

        pub(crate) fn changed(mut self, paths: &[&str]) -> Row {
            self.changed = Some(paths.iter().map(|p| (*p).to_owned()).collect());
            self
        }
    }

    fn apply(repo: &Repo, edits: &[Edit]) {
        for (path, text) in edits {
            match text {
                Some(text) => repo.write(path, text),
                None => repo.remove(path),
            }
        }
    }

    pub(crate) type Judge = fn(&Path, &str, &Head, Selection) -> Result<String, String>;

    pub(crate) fn judge(
        label: &str,
        standard: &BTreeMap<&'static str, String>,
        check: Judge,
        row: &Row,
    ) -> Option<String> {
        let repo = Repo::new(label);
        repo.init();
        let mut base = standard.clone();
        for (path, text) in &row.base {
            match text {
                Some(text) => base.insert(path, text.clone()),
                None => base.remove(path),
            };
        }
        for (path, text) in &base {
            repo.write(path, text);
        }
        repo.commit("base");
        let base_sha = repo.sha();
        apply(&repo, &row.head);
        let head = match row.at {
            At::Worktree => Head::Worktree,
            At::Commit | At::DetachedAtBase => {
                repo.commit("head");
                let head = Head::Commit(repo.sha());
                if matches!(row.at, At::DetachedAtBase) {
                    repo.git(&["checkout", "--quiet", "--detach", &base_sha]);
                }
                head
            }
        };
        let status = repo.status();
        let selection = match &row.changed {
            Some(paths) => Selection::Changed(paths),
            None => Selection::Every,
        };
        let found = check(&repo.0, &base_sha, &head, selection);
        if repo.status() != status {
            return Some(format!("{}: the check changed the working tree", row.name));
        }
        let wrong = match (&row.verdict, &found) {
            (Verdict::Pass(needles), Ok(detail)) => needles
                .iter()
                .find(|needle| !detail.contains(*needle))
                .map(|needle| format!("passed without {needle:?}")),
            (Verdict::Fail(needles), Err(message)) => needles
                .iter()
                .find(|needle| !message.contains(*needle))
                .map(|needle| format!("failed without {needle:?}")),
            (Verdict::Pass(_), Err(message)) => Some(format!("failed: {message}")),
            (Verdict::Fail(_), Ok(detail)) => Some(format!("passed: {detail}")),
        };
        wrong.map(|what| format!("{}: {what}; got {found:?}", row.name))
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::files;
    use super::*;
    use crate::scope::tests::Repo;

    #[test]
    fn a_file_belongs_to_the_package_in_its_longest_directory_prefix() {
        let manifests = files(&[
            ("Cargo.toml", "[package]\nname = \"root\"\n[workspace]\n"),
            ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            (
                "crates/a/tests/it/Cargo.toml",
                "[package]\nname = \"nested\"\n",
            ),
            ("crates/virtual/Cargo.toml", "[workspace]\nmembers = []\n"),
        ]);
        let packages = PackageMap::new(&manifests).unwrap();
        let owners: Vec<&str> = [
            "src/main.rs",
            "crates/a/src/lib.rs",
            "crates/a/tests/it/main.rs",
            "crates/ab/src/lib.rs",
            "crates/virtual/src/lib.rs",
        ]
        .iter()
        .map(|path| packages.owner(path))
        .collect();
        assert_eq!(owners, ["root", "a", "nested", "root", "root"]);
        let broken = files(&[("crates/x/Cargo.toml", "[package\n")]);
        let error = PackageMap::new(&broken)
            .err()
            .expect("a malformed manifest");
        assert!(error.contains("crates/x/Cargo.toml"), "{error}");
    }

    fn names_in_modules(root: &str, files: &Files) -> Vec<String> {
        let mut found = Vec::new();
        walk_modules(root, files, &mut |view| {
            for item in view.items {
                if let Item::Fn(f) = item {
                    found.push(format!(
                        "{}::{} @ {}",
                        view.module.join("::"),
                        f.sig.ident,
                        view.file
                    ));
                }
            }
        })
        .unwrap();
        found.sort();
        found
    }

    #[test]
    fn modules_resolve_to_their_files() {
        let tree = files(&[
            (
                "p/src/lib.rs",
                "fn top() {}\nmod a;\n#[path = \"elsewhere/b.rs\"]\nmod b;\nmod c { mod d; }\nmod e;\n#[cfg(windows)]\nmod gated;\n",
            ),
            (
                "p/src/a.rs",
                "fn in_a() {}\nmod inner;\nmod m {\n    #[path = \"y.rs\"]\n    mod z;\n}\n",
            ),
            ("p/src/a/inner.rs", "fn in_inner() {}\n"),
            ("p/src/a/m/y.rs", "fn in_y() {}\n"),
            ("p/src/elsewhere/b.rs", "fn in_b() {}\n"),
            ("p/src/c/d.rs", "fn in_d() {}\n"),
            ("p/src/e/mod.rs", "fn in_e() {}\nmod x;\n"),
            ("p/src/e/x.rs", "fn in_x() {}\n"),
        ]);
        assert_eq!(
            names_in_modules("p/src/lib.rs", &tree),
            [
                "::top @ p/src/lib.rs",
                "a::in_a @ p/src/a.rs",
                "a::inner::in_inner @ p/src/a/inner.rs",
                "a::m::z::in_y @ p/src/a/m/y.rs",
                "b::in_b @ p/src/elsewhere/b.rs",
                "c::d::in_d @ p/src/c/d.rs",
                "e::in_e @ p/src/e/mod.rs",
                "e::x::in_x @ p/src/e/x.rs",
            ]
        );
        let missing = files(&[("p/src/lib.rs", "mod nowhere;\n")]);
        let error = walk_modules("p/src/lib.rs", &missing, &mut |_| {}).unwrap_err();
        assert!(
            error.contains("p/src/lib.rs") && error.contains("p/src/nowhere.rs"),
            "{error}"
        );
    }

    #[test]
    fn string_literals_are_found_in_code_and_in_macro_bodies_but_not_in_docs() {
        let text = concat!(
            "/// documented \"in a doc\"\n",
            "#[doc = \"also a doc\"]\n",
            "#[path = \"attribute.rs\"]\n",
            "mod attributed;\n",
            "const TYPED: &str = \"typed\";\n",
            "fn f() {\n",
            "    let _ = include_str!(\"embedded.txt\");\n",
            "    nested!(inner!(\"deep\"), r#\"raw\"#, 7, b\"bytes\");\n",
            "}\n",
            "macro_rules! m { () => { #[doc = \"macro doc\"] fn g() {} } }\n",
        );
        let parsed = parse("a.rs", text).unwrap();
        let mut found = string_literals(&parsed);
        found.sort();
        assert_eq!(
            found,
            [
                ("attribute.rs".to_owned(), 3),
                ("deep".to_owned(), 8),
                ("embedded.txt".to_owned(), 7),
                ("raw".to_owned(), 8),
                ("typed".to_owned(), 5),
            ]
        );
    }

    #[test]
    fn a_syntax_error_names_its_file_and_line() {
        let error = parse("crates/x/src/lib.rs", "fn fine() {}\nfn broken( {\n")
            .err()
            .expect("a syntax error");
        assert!(
            error.starts_with("crates/x/src/lib.rs:2: does not parse: "),
            "{error}"
        );
    }

    #[test]
    fn a_commit_is_read_in_one_batch_like_the_worktree() {
        let repo = Repo::new("source-read");
        repo.init();
        repo.write("a/src/lib.rs", "fn a() {}\n");
        repo.write("a/notes.md", "not rust\n");
        repo.write("b.rs", "fn b() {}\n");
        repo.commit("first");
        let sha = repo.sha();
        repo.write("a/src/lib.rs", "fn changed_after() {}\n");
        repo.write("c.rs", "fn untracked() {}\n");
        repo.write(".gitignore", "ignored.rs\n");
        repo.write("ignored.rs", "fn ignored() {}\n");

        let at_commit = Head::Commit(sha);
        let paths = rust_files(&repo.0, &at_commit).unwrap();
        assert_eq!(paths, ["a/src/lib.rs", "b.rs"]);
        let committed = read_files(&repo.0, &at_commit, &paths).unwrap();
        assert_eq!(committed["a/src/lib.rs"], "fn a() {}\n");
        let in_tree = rust_files(&repo.0, &Head::Worktree).unwrap();
        assert_eq!(in_tree, ["a/src/lib.rs", "b.rs", "c.rs"]);
        let current = read_files(&repo.0, &Head::Worktree, &in_tree).unwrap();
        assert_eq!(current["a/src/lib.rs"], "fn changed_after() {}\n");
        let missing = read_files(&repo.0, &at_commit, &["gone.rs".to_owned()]).unwrap_err();
        assert!(missing.contains("gone.rs"), "{missing}");
    }
}
