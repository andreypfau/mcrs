use crate::metadata::Workspace;
use crate::mutation::attribution::TestId;
use crate::scope::{self, Head};
use crate::source::{self, Files, ModuleView};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprCall, ExprMethodCall, ExprUnsafe, ImplItem, Item, Local, Macro};
use syn::{Pat, Type, UseTree, Visibility};

const MAX_DEPTH: usize = 4;
const CONFORMANCE: &str = "conformance";
const SIZE_FUNCTIONS: [&str; 3] = ["size_of", "size_of_val", "align_of"];
const ASSERTIONS: [&str; 6] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
];
const ENCODERS: [&str; 16] = [
    "encode",
    "encode_section",
    "encode_container",
    "encode_registry_id",
    "encode_frame",
    "encode_delimited_ctx",
    "encode_delimited_patch",
    "to_bytes",
    "to_bytes_named",
    "to_bytes_unnamed",
    "to_gzip_bytes_vec",
    "to_nbt_tag",
    "to_nbt_compound",
    "write_gzip_compound_tag_to_bytes",
    "to_value",
    "to_vec",
];
const DECODERS: [&str; 12] = [
    "decode",
    "decode_container",
    "decode_registry_id",
    "decode_delimited_ctx",
    "decode_delimited_patch",
    "from_bytes",
    "from_bytes_unnamed",
    "from_gzip_bytes",
    "from_tag",
    "from_value",
    "from_slice",
    "read_gzip_compound_tag",
];

#[derive(Default, Clone)]
struct Shallow {
    idents: BTreeSet<String>,
    calls: BTreeSet<String>,
    followed: BTreeSet<String>,
    resolved: BTreeSet<Vec<String>>,
    strings: Vec<String>,
    assertion: bool,
    unsafe_block: bool,
    same_initialiser: bool,
}

impl Shallow {
    fn absorb(&mut self, other: &Shallow) {
        self.idents.extend(other.idents.iter().cloned());
        self.calls.extend(other.calls.iter().cloned());
        self.followed.extend(other.followed.iter().cloned());
        self.resolved.extend(other.resolved.iter().cloned());
        self.strings.extend(other.strings.iter().cloned());
        self.assertion |= other.assertion;
        self.unsafe_block |= other.unsafe_block;
        self.same_initialiser |= other.same_initialiser;
    }
}

pub(crate) struct Facts {
    name: String,
    shallow: Shallow,
}

#[derive(Default)]
pub(crate) struct Tables {
    unsafe_fns: BTreeSet<Vec<String>>,
    pub(crate) loaders: BTreeSet<String>,
    fixture_functions: BTreeSet<String>,
}

impl Tables {
    pub(crate) fn unsafe_fn_count(&self) -> usize {
        self.unsafe_fns.len()
    }
}

pub(crate) struct Row {
    pub category: &'static str,
    pub reason: &'static str,
    pub module: Option<&'static str>,
    pub matches: fn(&Facts, &Tables) -> bool,
}

fn size_pin(facts: &Facts, _: &Tables) -> bool {
    let s = &facts.shallow;
    s.assertion && SIZE_FUNCTIONS.iter().any(|name| s.idents.contains(*name))
}

fn ambiguity(facts: &Facts, _: &Tables) -> bool {
    let idents = &facts.shallow.idents;
    idents.contains("ambiguity_detection") || idents.contains("ScheduleBuildSettings")
}

fn determinism(facts: &Facts, _: &Tables) -> bool {
    facts.shallow.same_initialiser
}

fn unsafe_code(facts: &Facts, tables: &Tables) -> bool {
    facts.shallow.unsafe_block
        || facts
            .shallow
            .resolved
            .iter()
            .any(|path| tables.unsafe_fns.contains(path))
}

fn round_trip(facts: &Facts, _: &Tables) -> bool {
    let s = &facts.shallow;
    facts.name.contains("round_trip")
        || s.calls.contains("round_trip")
        || (s.assertion
            && ENCODERS.iter().any(|name| s.calls.contains(*name))
            && DECODERS.iter().any(|name| s.calls.contains(*name)))
}

fn is_assets_path(text: &str) -> bool {
    if text.contains(char::is_whitespace) {
        return false;
    }
    let parts: Vec<&str> = text.split('/').collect();
    parts
        .windows(2)
        .any(|pair| pair[0] == "assets" && !pair[1].is_empty())
}

fn corpus(facts: &Facts, tables: &Tables) -> bool {
    let s = &facts.shallow;
    s.strings.iter().any(|text| is_assets_path(text))
        || s.calls.iter().any(|name| tables.loaders.contains(name))
        || s.resolved.iter().any(|path| {
            matches!(path.as_slice(), [krate, function]
                if krate == FIXTURE_LOADER_CRATE && tables.fixture_functions.contains(function))
        })
}

pub(crate) const ROWS: [Row; 6] = [
    Row {
        category: "size-pin",
        reason: "a size pin compares a layout constant, which no mutant of the crate's code changes",
        module: None,
        matches: size_pin,
    },
    Row {
        category: "ambiguity",
        reason: "an ambiguity test checks how the schedule is ordered, which no mutant of a function body changes",
        module: None,
        matches: ambiguity,
    },
    Row {
        category: "determinism",
        reason: "a determinism test compares two runs of the same input, which stay equal under a mutant that is itself deterministic",
        module: None,
        matches: determinism,
    },
    Row {
        category: "unsafe",
        reason: "cargo-mutants never mutates an unsafe function, so the code such a test exercises has no mutant to catch",
        module: None,
        matches: unsafe_code,
    },
    Row {
        category: "round-trip",
        reason: "a round-trip test passes whenever decoding stays the inverse of encoding, so it seldom catches a mutant the value tests miss",
        module: None,
        matches: round_trip,
    },
    Row {
        category: "corpus",
        reason: "a corpus test checks code against the shipped data, so a mutant in code the data never reaches survives it",
        module: None,
        matches: corpus,
    },
];

#[derive(Clone, Debug)]
pub(crate) struct TargetSpec {
    pub package: String,
    pub binary: String,
    pub root: String,
    pub lib: bool,
}

const FIXTURE_LOADER_CRATE: &str = "mcrs_minecraft_fixtures";

const LIB_KINDS: [&str; 6] = ["lib", "rlib", "proc-macro", "cdylib", "dylib", "staticlib"];

pub(crate) fn target_specs(workspace: &Workspace) -> Vec<TargetSpec> {
    let mut specs = Vec::new();
    for package in &workspace.packages {
        for target in &package.targets {
            let has = |kind: &str| target.kinds.iter().any(|k| k == kind);
            let (binary, lib) = if target.kinds.iter().any(|k| LIB_KINDS.contains(&k.as_str())) {
                (package.name.clone(), true)
            } else if has("test") {
                (format!("{}::{}", package.name, target.name), false)
            } else if has("bin") {
                (format!("{}::bin/{}", package.name, target.name), false)
            } else {
                continue;
            };
            specs.push(TargetSpec {
                package: package.name.clone(),
                binary,
                root: target.path.clone(),
                lib,
            });
        }
    }
    specs
}

fn absolutize(krate: &str, module: &[String], path: &[String]) -> Vec<String> {
    let mut absolute = Vec::new();
    let mut rest = path;
    match path.first().map(String::as_str) {
        Some("crate") => {
            absolute.push(krate.to_owned());
            rest = &path[1..];
        }
        Some("self") => {
            absolute.push(krate.to_owned());
            absolute.extend(module.iter().cloned());
            rest = &path[1..];
        }
        Some("super") => {
            let mut up = module.len();
            while rest.first().is_some_and(|p| p == "super") {
                up = up.saturating_sub(1);
                rest = &rest[1..];
            }
            absolute.push(krate.to_owned());
            absolute.extend(module[..up].iter().cloned());
        }
        _ => {}
    }
    absolute.extend(rest.iter().cloned());
    absolute
}

struct Scope {
    krate: String,
    module: Vec<String>,
    uses: BTreeMap<String, Vec<String>>,
    globs: Vec<Vec<String>>,
}

type Import = (Option<String>, Vec<String>);

fn flatten(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<Import>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten(&path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if ident == "self" {
                out.push((prefix.last().cloned(), prefix.clone()));
            } else {
                let mut path = prefix.clone();
                path.push(ident.clone());
                out.push((Some(ident), path));
            }
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            path.push(rename.ident.to_string());
            out.push((Some(rename.rename.to_string()), path));
        }
        UseTree::Glob(_) => out.push((None, prefix.clone())),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten(item, prefix, out);
            }
        }
    }
}

impl Scope {
    fn of(krate: &str, view: &ModuleView) -> Scope {
        let mut scope = Scope {
            krate: krate.to_owned(),
            module: view.module.to_vec(),
            uses: BTreeMap::new(),
            globs: Vec::new(),
        };
        let mut imports = Vec::new();
        for item in view.items {
            if let Item::Use(import) = item {
                flatten(&import.tree, &mut Vec::new(), &mut imports);
            }
        }
        for (alias, path) in imports {
            let absolute = absolutize(krate, view.module, &path);
            match alias {
                Some(alias) => {
                    scope.uses.insert(alias, absolute);
                }
                None => scope.globs.push(absolute),
            }
        }
        scope
    }

    // chisle: a path through a `pub use` re-export names the re-exporting module, not the declaring one, so it
    // does not match; a `use` inside a function body is not read. Resolving either needs name resolution.
    fn resolve(&self, segments: &[String]) -> Vec<Vec<String>> {
        let first = segments[0].as_str();
        if matches!(first, "crate" | "self" | "super") {
            return vec![absolutize(&self.krate, &self.module, segments)];
        }
        if let Some(base) = self.uses.get(first) {
            let mut path = base.clone();
            path.extend(segments[1..].iter().cloned());
            return vec![path];
        }
        let mut local = vec![self.krate.clone()];
        local.extend(self.module.iter().cloned());
        local.extend(segments.iter().cloned());
        let mut candidates = vec![local];
        for glob in &self.globs {
            let mut path = glob.clone();
            path.extend(segments.iter().cloned());
            candidates.push(path);
        }
        candidates.push(segments.to_vec());
        candidates
    }
}

struct Binding {
    name: String,
    initialiser: String,
    reads: BTreeSet<String>,
}

struct Collector<'a> {
    scope: &'a Scope,
    out: Shallow,
    bindings: Vec<Binding>,
    mutable: BTreeSet<String>,
    assertions: Vec<BTreeSet<String>>,
}

fn is_punct(token: Option<&TokenTree>, wanted: char) -> bool {
    matches!(token, Some(TokenTree::Punct(p)) if p.as_char() == wanted)
}

fn is_group(token: Option<&TokenTree>, wanted: Delimiter) -> bool {
    matches!(token, Some(TokenTree::Group(g)) if g.delimiter() == wanted)
}

fn is_call(tokens: &[TokenTree], at: usize) -> bool {
    let mut next = at + 1;
    if is_punct(tokens.get(next), ':')
        && is_punct(tokens.get(next + 1), ':')
        && is_punct(tokens.get(next + 2), '<')
    {
        let mut depth = 0;
        next += 2;
        while let Some(token) = tokens.get(next) {
            if is_punct(Some(token), '<') {
                depth += 1;
            } else if is_punct(Some(token), '>') {
                depth -= 1;
                if depth == 0 {
                    next += 1;
                    break;
                }
            }
            next += 1;
        }
    }
    is_group(tokens.get(next), Delimiter::Parenthesis)
}

fn path_before(tokens: &[TokenTree], at: usize) -> Vec<String> {
    let mut segments = vec![ident_text(&tokens[at])];
    let mut index = at;
    while index >= 3
        && is_punct(tokens.get(index - 1), ':')
        && is_punct(tokens.get(index - 2), ':')
        && matches!(tokens.get(index - 3), Some(TokenTree::Ident(_)))
    {
        segments.push(ident_text(&tokens[index - 3]));
        index -= 3;
    }
    segments.reverse();
    segments
}

fn ident_text(token: &TokenTree) -> String {
    token.to_string()
}

impl<'a> Collector<'a> {
    fn new(scope: &'a Scope) -> Collector<'a> {
        Collector {
            scope,
            out: Shallow::default(),
            bindings: Vec::new(),
            mutable: BTreeSet::new(),
            assertions: Vec::new(),
        }
    }

    fn call(&mut self, segments: Vec<String>) {
        let Some(last) = segments.last().cloned() else {
            return;
        };
        self.out.calls.insert(last.clone());
        self.out.followed.insert(last);
        self.out.resolved.extend(self.scope.resolve(&segments));
    }

    fn token(&mut self, tokens: &[TokenTree], at: usize) {
        match &tokens[at] {
            TokenTree::Ident(ident) => {
                let name = ident.to_string();
                self.out.idents.insert(name.clone());
                if name == "unsafe" && is_group(tokens.get(at + 1), Delimiter::Brace) {
                    self.out.unsafe_block = true;
                }
                if ASSERTIONS.contains(&name.as_str()) && is_punct(tokens.get(at + 1), '!') {
                    self.out.assertion = true;
                }
                if is_call(tokens, at) {
                    if at > 0 && is_punct(tokens.get(at - 1), '.') {
                        self.out.calls.insert(name);
                    } else {
                        self.call(path_before(tokens, at));
                    }
                }
            }
            TokenTree::Literal(literal) => {
                if let Some(text) = source::string_literal(literal) {
                    self.out.strings.push(text);
                }
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Shallow {
        let pure: Vec<&Binding> = self
            .bindings
            .iter()
            .filter(|b| !self.mutable.contains(&b.name) && b.reads.is_disjoint(&self.mutable))
            .collect();
        self.out.same_initialiser = pure.iter().enumerate().any(|(first, a)| {
            pure[first + 1..].iter().any(|b| {
                a.name != b.name
                    && a.initialiser == b.initialiser
                    && self
                        .assertions
                        .iter()
                        .any(|idents| idents.contains(&a.name) && idents.contains(&b.name))
            })
        });
        self.out
    }
}

fn idents_of(stream: &TokenStream) -> BTreeSet<String> {
    let mut idents = BTreeSet::new();
    source::walk_tokens(stream, &mut |tokens, at| {
        if let TokenTree::Ident(ident) = &tokens[at] {
            idents.insert(ident.to_string());
        }
    });
    idents
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        self.out.idents.insert(ident.to_string());
    }

    fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
        self.out.strings.push(literal.value());
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if let Expr::Path(path) = &*call.func {
            let segments = path.path.segments.iter().map(|s| s.ident.to_string());
            self.call(segments.collect());
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        self.out.calls.insert(call.method.to_string());
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_unsafe(&mut self, block: &'ast ExprUnsafe) {
        self.out.unsafe_block = true;
        visit::visit_expr_unsafe(self, block);
    }

    fn visit_local(&mut self, local: &'ast Local) {
        if let Pat::Ident(binding) = &local.pat {
            let name = binding.ident.to_string();
            if binding.mutability.is_some() {
                self.mutable.insert(name.clone());
            }
            if let Some(init) = &local.init {
                let stream = init.expr.to_token_stream();
                let initialiser = stream.to_string();
                if initialiser.contains('(') {
                    self.bindings.push(Binding {
                        name,
                        reads: idents_of(&stream),
                        initialiser,
                    });
                }
            }
        }
        visit::visit_local(self, local);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        visit::visit_macro(self, mac);
        let asserts = mac
            .path
            .segments
            .last()
            .is_some_and(|s| ASSERTIONS.contains(&s.ident.to_string().as_str()));
        if asserts {
            self.out.assertion = true;
            self.assertions.push(idents_of(&mac.tokens));
        }
        source::walk_tokens(&mac.tokens, &mut |tokens, at| self.token(tokens, at));
    }
}

fn collect(scope: &Scope, visit: impl FnOnce(&mut Collector)) -> Shallow {
    let mut collector = Collector::new(scope);
    visit(&mut collector);
    collector.finish()
}

struct Callable {
    name: String,
    public: bool,
    shallow: Shallow,
}

struct TestRecord {
    name: String,
    module: Vec<String>,
    file: String,
    callable: usize,
}

struct Index {
    target: TargetSpec,
    callables: Vec<Callable>,
    functions: BTreeMap<String, Vec<usize>>,
    constants: BTreeMap<String, Vec<usize>>,
    tests: Vec<TestRecord>,
    unsafe_declared: BTreeSet<Vec<String>>,
    doctests: usize,
}

impl Index {
    fn build(target: &TargetSpec, files: &Files) -> Result<Index, String> {
        let mut index = Index {
            target: target.clone(),
            callables: Vec::new(),
            functions: BTreeMap::new(),
            constants: BTreeMap::new(),
            tests: Vec::new(),
            unsafe_declared: BTreeSet::new(),
            doctests: 0,
        };
        source::walk_modules(&target.root, files, &mut |view| index.add(view))?;
        Ok(index)
    }

    fn register(&mut self, name: String, public: bool, constant: bool, shallow: Shallow) -> usize {
        let at = self.callables.len();
        let map = if constant {
            &mut self.constants
        } else {
            &mut self.functions
        };
        map.entry(name.clone()).or_default().push(at);
        self.callables.push(Callable {
            name,
            public,
            shallow,
        });
        at
    }

    fn declared_path(&self, module: &[String], tail: &[&str]) -> Vec<String> {
        let mut path = vec![self.target.binary.clone()];
        path.extend(module.iter().cloned());
        path.extend(tail.iter().map(|s| (*s).to_owned()));
        path
    }

    fn add(&mut self, view: &ModuleView) {
        let scope = Scope::of(&self.target.binary, view);
        for item in view.items {
            match item {
                Item::Fn(function) => {
                    let name = function.sig.ident.to_string();
                    let shallow = collect(&scope, |c| c.visit_block(&function.block));
                    let public = matches!(function.vis, Visibility::Public(_));
                    let at = self.register(name.clone(), public, false, shallow);
                    if function.sig.unsafety.is_some() {
                        let path = self.declared_path(view.module, &[&name]);
                        self.unsafe_declared.insert(path);
                    }
                    if source::is_test(&function.attrs) {
                        self.tests.push(TestRecord {
                            name,
                            module: view.module.to_vec(),
                            file: view.file.to_owned(),
                            callable: at,
                        });
                    }
                }
                // chisle: only free functions are followed as helpers; a helper that is a method needs type
                // resolution to find, so a test that reaches its rule only through one is judged on its own body.
                Item::Impl(block) => {
                    let Type::Path(owner) = &*block.self_ty else {
                        continue;
                    };
                    let Some(owner) = owner.path.segments.last().map(|s| s.ident.to_string())
                    else {
                        continue;
                    };
                    for member in &block.items {
                        if let ImplItem::Fn(method) = member
                            && method.sig.unsafety.is_some()
                        {
                            let name = method.sig.ident.to_string();
                            let path = self.declared_path(view.module, &[&owner, &name]);
                            self.unsafe_declared.insert(path);
                        }
                    }
                }
                Item::Const(constant) => {
                    let shallow = collect(&scope, |c| c.visit_expr(&constant.expr));
                    self.register(constant.ident.to_string(), false, true, shallow);
                }
                Item::Static(constant) => {
                    let shallow = collect(&scope, |c| c.visit_expr(&constant.expr));
                    self.register(constant.ident.to_string(), false, true, shallow);
                }
                _ => {}
            }
        }
        if self.target.lib {
            self.doctests += count_runnable_fences(view);
        }
    }

    fn closure(&self, start: usize) -> Shallow {
        let mut seen = BTreeSet::from([start]);
        let mut merged = Shallow::default();
        let mut frontier = vec![start];
        for depth in 0..=MAX_DEPTH {
            let mut next = Vec::new();
            for &at in &frontier {
                let shallow = &self.callables[at].shallow;
                merged.absorb(shallow);
                if depth == MAX_DEPTH {
                    continue;
                }
                let named = shallow
                    .followed
                    .iter()
                    .filter_map(|name| self.functions.get(name));
                let read = shallow
                    .idents
                    .iter()
                    .filter_map(|name| self.constants.get(name));
                for &found in named.chain(read).flatten() {
                    if seen.insert(found) {
                        next.push(found);
                    }
                }
            }
            frontier = next;
        }
        merged
    }
}

struct Fences {
    count: usize,
    lines: Vec<String>,
}

fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let line = line.trim_start();
    let first = line.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let width = line.chars().take_while(|c| *c == first).count();
    (width >= 3).then(|| (first, width, line[width..].trim()))
}

fn runnable(info: &str) -> bool {
    info.split([',', ' ', '\t'])
        .filter(|word| !word.is_empty())
        .all(|word| {
            matches!(word, "rust" | "should_panic")
                || word
                    .strip_prefix("edition")
                    .is_some_and(|year| year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()))
        })
}

impl Fences {
    fn flush(&mut self) {
        let mut open: Option<(char, usize)> = None;
        for line in std::mem::take(&mut self.lines) {
            let Some((mark, width, info)) = fence_marker(&line) else {
                continue;
            };
            match open {
                Some((kind, opened)) if kind == mark && width >= opened && info.is_empty() => {
                    open = None;
                }
                Some(_) => {}
                None => {
                    open = Some((mark, width));
                    if runnable(info) {
                        self.count += 1;
                    }
                }
            }
        }
    }

    fn attrs(&mut self, attrs: &[Attribute]) {
        self.lines.extend(attrs.iter().filter_map(source::doc_line));
    }
}

impl<'ast> Visit<'ast> for Fences {
    fn visit_item(&mut self, item: &'ast Item) {
        self.flush();
        if let Item::Mod(module) = item {
            self.attrs(&module.attrs);
            self.flush();
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        self.flush();
        visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        self.flush();
        visit::visit_trait_item(self, item);
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        self.flush();
        visit::visit_field(self, field);
    }

    fn visit_variant(&mut self, variant: &'ast syn::Variant) {
        self.flush();
        visit::visit_variant(self, variant);
    }

    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.lines.extend(source::doc_line(attr));
    }
}

fn count_runnable_fences(view: &ModuleView) -> usize {
    let mut fences = Fences {
        count: 0,
        lines: Vec::new(),
    };
    fences.attrs(view.attrs);
    fences.flush();
    for item in view.items {
        fences.visit_item(item);
    }
    fences.flush();
    fences.count
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Classified {
    pub package: String,
    pub id: TestId,
    pub module: Vec<String>,
    pub file: String,
    pub category: Option<&'static str>,
    pub reason: &'static str,
    pub measured: bool,
    pub unclassified: bool,
}

pub(crate) struct Report {
    pub tests: Vec<Classified>,
    pub doctests: BTreeMap<String, usize>,
    pub tables: Tables,
}

pub(crate) fn not_measured_markers(nextest: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Config {
        profile: BTreeMap<String, Profile>,
    }
    #[derive(Deserialize)]
    struct Profile {
        #[serde(rename = "default-filter")]
        default_filter: Option<String>,
    }
    let config: Config = toml::from_str(nextest).map_err(|e| e.message().to_owned())?;
    let filter = config
        .profile
        .get("default")
        .and_then(|profile| profile.default_filter.as_deref())
        .ok_or("the default nextest profile has no default-filter")?;
    let shape = Regex::new(r"\(\^\|::\)\(([a-z_|]+)\)::").expect("a valid pattern");
    let found = shape.captures(filter).ok_or_else(|| {
        format!("the default-filter {filter:?} does not name its excluded modules as test(/(^|::)(a|b)::/)")
    })?;
    Ok(found[1].split('|').map(str::to_owned).collect())
}

pub(crate) fn classify(
    files: &Files,
    targets: &[TargetSpec],
    markers: &[String],
    rows: &[Row],
) -> Result<Report, String> {
    let indexes = targets
        .iter()
        .map(|target| Index::build(target, files))
        .collect::<Result<Vec<_>, _>>()?;
    let mut tables = Tables::default();
    for index in &indexes {
        tables
            .unsafe_fns
            .extend(index.unsafe_declared.iter().cloned());
    }
    for index in indexes.iter().filter(|index| index.target.lib) {
        for (at, callable) in index.callables.iter().enumerate() {
            if callable.public && index.functions.contains_key(&callable.name) {
                if index.target.package == FIXTURE_LOADER_CRATE {
                    tables.fixture_functions.insert(callable.name.clone());
                }
                let reads = index
                    .closure(at)
                    .strings
                    .iter()
                    .any(|text| is_assets_path(text));
                if reads {
                    tables.loaders.insert(callable.name.clone());
                }
            }
        }
    }
    let mut report = Report {
        tests: Vec::new(),
        doctests: BTreeMap::new(),
        tables,
    };
    for index in &indexes {
        *report
            .doctests
            .entry(index.target.package.clone())
            .or_default() += index.doctests;
        for test in &index.tests {
            let facts = Facts {
                name: test.name.clone(),
                shallow: index.closure(test.callable),
            };
            let measured = !test.module.iter().any(|segment| markers.contains(segment));
            let matched = rows.iter().find(|row| {
                row.module
                    .is_none_or(|wanted| test.module.iter().any(|segment| segment == wanted))
                    && (row.matches)(&facts, &report.tables)
            });
            let protected = measured.then_some(matched).flatten();
            report.tests.push(Classified {
                package: index.target.package.clone(),
                id: TestId {
                    binary: index.target.binary.clone(),
                    name: test
                        .module
                        .iter()
                        .map(String::as_str)
                        .chain([test.name.as_str()])
                        .collect::<Vec<_>>()
                        .join("::"),
                },
                module: test.module.clone(),
                file: test.file.clone(),
                category: protected.map(|row| row.category),
                reason: protected.map_or("", |row| row.reason),
                measured,
                unclassified: measured
                    && protected.is_none()
                    && test.module.iter().any(|segment| segment == CONFORMANCE),
            });
        }
    }
    report.tests.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(report)
}

pub(crate) fn workspace_report(dir: &Path, workspace: &Workspace) -> Result<Report, String> {
    let paths = source::rust_files(dir, &Head::Worktree)?;
    let files = source::read_files(dir, &Head::Worktree, &paths)?;
    let nextest = std::fs::read_to_string(dir.join(".config/nextest.toml"))
        .map_err(|e| format!("could not read .config/nextest.toml: {e}"))?;
    let markers =
        not_measured_markers(&nextest).map_err(|e| format!(".config/nextest.toml: {e}"))?;
    classify(&files, &target_specs(workspace), &markers, &ROWS)
}

pub(crate) fn report_at(repo: &Path, workspace: &Workspace, rev: &str) -> Result<Report, String> {
    let head = Head::Commit(rev.to_owned());
    let paths = source::rust_files(repo, &head)?;
    let files = source::read_files(repo, &head, &paths)?;
    let nextest = scope::read_at(repo, rev, ".config/nextest.toml")?
        .ok_or_else(|| format!(".config/nextest.toml is missing at {}", scope::short(rev)))?;
    let markers =
        not_measured_markers(&nextest).map_err(|e| format!(".config/nextest.toml: {e}"))?;
    let targets: Vec<TargetSpec> = target_specs(workspace)
        .into_iter()
        .filter(|target| files.contains_key(&target.root))
        .collect();
    classify(&files, &targets, &markers, &ROWS)
}

pub(crate) fn changed_counts(
    crates: &[String],
    base: (&str, &Report),
    head: (&str, &Report),
) -> String {
    let mut text = String::from("### Protected tests of crates whose tests changed\n\n");
    for package in crates {
        for (label, (rev, report)) in [("base", base), ("head", head)] {
            text.push_str(&format!(
                "- at the {label} {}: {}\n",
                scope::short(rev),
                summary_line(report, package, Some(package), &ROWS)
            ));
        }
    }
    text
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Protection {
    pub protected: Vec<(TestId, String)>,
    pub unclassified: Vec<TestId>,
}

pub(crate) fn protection(report: &Report, package: &str) -> Protection {
    let mut found = Protection::default();
    for test in report.tests.iter().filter(|t| t.package == package) {
        if let Some(category) = test.category {
            found.protected.push((test.id.clone(), category.to_owned()));
        }
        if test.unclassified {
            found.unclassified.push(test.id.clone());
        }
    }
    found
}

pub(crate) fn summary_line(
    report: &Report,
    label: &str,
    package: Option<&str>,
    rows: &[Row],
) -> String {
    let own: Vec<&Classified> = report
        .tests
        .iter()
        .filter(|t| package.is_none_or(|p| t.package == p))
        .collect();
    let by_category: Vec<String> = rows
        .iter()
        .map(|row| {
            let count = own
                .iter()
                .filter(|t| t.category == Some(row.category))
                .count();
            format!("{} {count}", row.category)
        })
        .collect();
    let protected = own.iter().filter(|t| t.category.is_some()).count();
    let doctests: usize = report
        .doctests
        .iter()
        .filter(|(name, _)| package.is_none_or(|p| name.as_str() == p))
        .map(|(_, count)| count)
        .sum();
    format!(
        "{label}: {} tests, {protected} protected ({}), {doctests} doctests, {} not measured, {} unclassified conformance",
        own.len(),
        by_category.join(", "),
        own.iter().filter(|t| !t.measured).count(),
        own.iter().filter(|t| t.unclassified).count(),
    )
}

pub(crate) fn table_lines(report: &Report) -> Vec<String> {
    let loaders: Vec<&str> = report.tables.loaders.iter().map(String::as_str).collect();
    vec![
        format!(
            "unsafe functions declared in the workspace: {}",
            report.tables.unsafe_fn_count()
        ),
        format!(
            "corpus loaders found in the workspace ({}): {}",
            loaders.len(),
            loaders.join(", ")
        ),
    ]
}

pub(crate) fn protected_lines(report: &Report, package: &str) -> Vec<String> {
    report
        .tests
        .iter()
        .filter(|t| t.package == package && t.category.is_some())
        .map(|t| {
            format!(
                "    {} {}  [{}] {}",
                t.id.binary,
                t.id.name,
                t.category.unwrap_or_default(),
                t.reason
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    type Target = (&'static str, &'static str, &'static str, bool);

    struct Fixture {
        files: Vec<(&'static str, &'static str)>,
        targets: Vec<Target>,
        test: (&'static str, &'static str),
    }

    fn lib(source: &'static str, test: &'static str) -> Fixture {
        Fixture {
            files: vec![("pkg/src/lib.rs", source)],
            targets: vec![("pkg", "pkg", "pkg/src/lib.rs", true)],
            test: ("pkg", test),
        }
    }

    fn files_of(rows: &[(&str, &str)]) -> Files {
        rows.iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    fn specs(targets: &[Target]) -> Vec<TargetSpec> {
        targets
            .iter()
            .map(|(package, binary, root, lib)| TargetSpec {
                package: (*package).to_owned(),
                binary: (*binary).to_owned(),
                root: (*root).to_owned(),
                lib: *lib,
            })
            .collect()
    }

    fn report_of(fixture: &Fixture, rows: &[Row]) -> Report {
        let markers = ["exhaustive".to_owned()];
        classify(
            &files_of(&fixture.files),
            &specs(&fixture.targets),
            &markers,
            rows,
        )
        .unwrap()
    }

    fn test_in<'a>(report: &'a Report, (binary, name): (&str, &str)) -> &'a Classified {
        report
            .tests
            .iter()
            .find(|t| t.id.binary == binary && t.id.name == name)
            .unwrap_or_else(|| {
                let found: Vec<_> = report.tests.iter().map(|t| &t.id).collect();
                panic!("{binary} {name} not among {found:?}")
            })
    }

    fn category_of(fixture: &Fixture) -> Option<&'static str> {
        test_in(&report_of(fixture, &ROWS), fixture.test).category
    }

    #[test]
    fn the_protected_count_is_shown_at_the_base_and_at_the_head() {
        let plain = lib("#[test]\nfn plain() {\n    assert_eq!(1, 1);\n}\n", "plain");
        let pinned = lib(
            "#[test]\nfn plain() {\n    assert_eq!(1, 1);\n}\n\n#[test]\nfn pin() {\n    assert!(size_of::<A>() <= 8);\n}\n",
            "pin",
        );
        let (before, after) = (report_of(&plain, &ROWS), report_of(&pinned, &ROWS));
        let text = changed_counts(
            &["pkg".to_owned()],
            ("aaaaaaaaaaaaaaaa", &before),
            ("bbbbbbbbbbbbbbbb", &after),
        );
        let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(
            lines[0].starts_with("- at the base aaaaaaaaa: pkg: 1 tests, 0 protected (size-pin 0"),
            "{text}"
        );
        assert!(
            lines[1].starts_with("- at the head bbbbbbbbb: pkg: 2 tests, 1 protected (size-pin 1"),
            "{text}"
        );
    }

    const LOWLEVEL: Target = ("lowlevel", "lowlevel", "lowlevel/src/lib.rs", true);

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture and one counterexample for each protected category, in a single table"
    )]
    fn each_protected_category_matches_its_fixture_and_not_its_counterexample() {
        struct Case {
            category: &'static str,
            positives: Vec<(&'static str, Fixture)>,
            negative: Fixture,
        }
        let cases = vec![
            Case {
                category: "size-pin",
                positives: vec![
                    (
                        "one assertion",
                        lib(
                            "#[test]\nfn the_section_stays_small() {\n    assert!(size_of::<Section>() <= 64);\n}\n",
                            "the_section_stays_small",
                        ),
                    ),
                    (
                        "a conformance table of assertions",
                        lib(
                            "#[cfg(test)]\nmod conformance {\n    use std::mem::size_of;\n\n    #[test]\n    fn the_pins() {\n        assert!(size_of::<A>() <= 8, \"A\");\n        assert!(size_of::<B>() <= 16, \"B\");\n        assert!(size_of::<C>() <= 24, \"C\");\n    }\n}\n",
                            "conformance::the_pins",
                        ),
                    ),
                    (
                        "a table read through a constant",
                        lib(
                            "const PINS: [(&str, usize, usize); 2] = [(\"A\", size_of::<A>(), 8), (\"B\", size_of::<B>(), 16)];\n\n#[test]\nfn pins() {\n    for (name, size, most) in PINS {\n        assert!(size <= most, \"{name}\");\n    }\n}\n",
                            "pins",
                        ),
                    ),
                ],
                negative: lib(
                    "#[test]\nfn measures() {\n    let n = size_of::<T>();\n    println!(\"{n}\");\n}\n",
                    "measures",
                ),
            },
            Case {
                category: "ambiguity",
                positives: vec![(
                    "settings in a fixture file of the same tests tree",
                    Fixture {
                        files: vec![
                            (
                                "pkg/tests/it/main.rs",
                                "mod fixture;\n\nmod schedule {\n    #[test]\n    fn no_ambiguity() {\n        crate::fixture::ambiguity_app();\n    }\n}\n",
                            ),
                            (
                                "pkg/tests/it/fixture.rs",
                                "pub fn ambiguity_app() {\n    let settings = ScheduleBuildSettings {\n        ambiguity_detection: LogLevel::Error,\n        ..Default::default()\n    };\n    build(settings);\n}\n",
                            ),
                        ],
                        targets: vec![("pkg", "pkg::it", "pkg/tests/it/main.rs", false)],
                        test: ("pkg::it", "schedule::no_ambiguity"),
                    },
                )],
                negative: lib(
                    "mod conformance {\n    #[test]\n    fn census() {\n        assert_eq!(heap_bytes(&table).len(), 3);\n    }\n}\n",
                    "conformance::census",
                ),
            },
            Case {
                category: "determinism",
                positives: vec![(
                    "two identical runs compared",
                    lib(
                        "#[test]\nfn same_seed_same_world() {\n    let a = generate(7);\n    let b = generate(7);\n    assert_eq!(a, b);\n}\n",
                        "same_seed_same_world",
                    ),
                )],
                negative: lib(
                    "#[test]\nfn other_seed_other_world() {\n    let a = generate(7);\n    let b = generate(8);\n    assert_ne!(a, b);\n}\n",
                    "other_seed_other_world",
                ),
            },
            Case {
                category: "unsafe",
                positives: vec![
                    (
                        "an unsafe block",
                        lib(
                            "#[test]\nfn reads_raw() {\n    let v = unsafe { read_raw(pointer) };\n    assert_eq!(v, 1);\n}\n",
                            "reads_raw",
                        ),
                    ),
                    (
                        "a call to a workspace unsafe fn, matched by its resolved path",
                        Fixture {
                            files: vec![
                                ("lowlevel/src/lib.rs", "pub unsafe fn danger() {}\n"),
                                (
                                    "pkg/src/lib.rs",
                                    "use lowlevel::danger;\n\n#[test]\nfn calls_it() {\n    danger();\n}\n",
                                ),
                            ],
                            targets: vec![LOWLEVEL, ("pkg", "pkg", "pkg/src/lib.rs", true)],
                            test: ("pkg", "calls_it"),
                        },
                    ),
                ],
                negative: Fixture {
                    files: vec![
                        ("lowlevel/src/lib.rs", "pub unsafe fn danger() {}\n"),
                        ("other/src/lib.rs", "pub fn danger() {}\n"),
                        (
                            "pkg/src/lib.rs",
                            "use other::danger;\n\n#[test]\nfn calls_the_safe_one() {\n    danger();\n}\n",
                        ),
                    ],
                    targets: vec![
                        LOWLEVEL,
                        ("other", "other", "other/src/lib.rs", true),
                        ("pkg", "pkg", "pkg/src/lib.rs", true),
                    ],
                    test: ("pkg", "calls_the_safe_one"),
                },
            },
            Case {
                category: "round-trip",
                positives: vec![
                    (
                        "a test named for the round trip",
                        lib(
                            "#[test]\nfn a_value_round_trips() {\n    check();\n}\n",
                            "a_value_round_trips",
                        ),
                    ),
                    (
                        "an encode and a decode compared",
                        lib(
                            "#[test]\nfn survives() {\n    let bytes = encode(&value);\n    let back = decode(&bytes);\n    assert_eq!(value, back);\n}\n",
                            "survives",
                        ),
                    ),
                ],
                negative: lib(
                    "#[test]\nfn writes() {\n    let bytes = encode(&value);\n    assert_eq!(bytes.len(), 3);\n}\n",
                    "writes",
                ),
            },
            Case {
                category: "corpus",
                positives: vec![
                    (
                        "a path under assets",
                        lib(
                            "#[test]\nfn reads_the_version() {\n    let text = std::fs::read_to_string(\"../../assets/minecraft/version.json\");\n    assert!(text.is_ok());\n}\n",
                            "reads_the_version",
                        ),
                    ),
                    (
                        "an include of such a path",
                        lib(
                            "#[test]\nfn embeds() {\n    let bytes = include_bytes!(\"../../../assets/x.bin\");\n    assert!(!bytes.is_empty());\n}\n",
                            "embeds",
                        ),
                    ),
                    (
                        "a call to a loader the workspace declares",
                        Fixture {
                            files: vec![
                                (
                                    "data/src/lib.rs",
                                    "pub fn load_all() -> String {\n    std::fs::read_to_string(\"../../assets/a.json\").unwrap()\n}\n",
                                ),
                                (
                                    "pkg/src/lib.rs",
                                    "#[test]\nfn uses_a_loader() {\n    let text = data::load_all();\n    assert!(!text.is_empty());\n}\n",
                                ),
                            ],
                            targets: vec![
                                ("data", "data", "data/src/lib.rs", true),
                                ("pkg", "pkg", "pkg/src/lib.rs", true),
                            ],
                            test: ("pkg", "uses_a_loader"),
                        },
                    ),
                ],
                negative: lib(
                    "#[test]\nfn mentions() {\n    let note = \"see the assets crate\";\n    let other = \"my_assets/data\";\n    assert!(note.len() > other.len());\n}\n",
                    "mentions",
                ),
            },
        ];
        let listed: Vec<&str> = ROWS.iter().map(|row| row.category).collect();
        let tabled: Vec<&str> = cases.iter().map(|case| case.category).collect();
        assert_eq!(tabled, listed, "one case per row of the rule table");
        for case in cases {
            for (name, fixture) in &case.positives {
                assert_eq!(
                    category_of(fixture),
                    Some(case.category),
                    "{}: {name}",
                    case.category
                );
            }
            assert_eq!(
                category_of(&case.negative),
                None,
                "{}: counterexample",
                case.category
            );
        }
    }

    #[test]
    fn a_loader_call_marks_a_test_that_reads_oracle_fixtures() {
        let loads = |body: &'static str| {
            let mut fixture = lib(body, "reads");
            fixture.files.push((
                "crates/mcrs_minecraft_fixtures/src/lib.rs",
                "pub struct Fixture;\n\npub fn fixture(name: &str) -> Fixture {\n    Fixture\n}\n",
            ));
            fixture.targets.push((
                FIXTURE_LOADER_CRATE,
                FIXTURE_LOADER_CRATE,
                "crates/mcrs_minecraft_fixtures/src/lib.rs",
                true,
            ));
            fixture
        };
        let positives = [
            (
                "a call through an imported name",
                "use mcrs_minecraft_fixtures::fixture;\n\n#[test]\nfn reads() {\n    let dump = fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a call through the full path",
                "#[test]\nfn reads() {\n    let dump = mcrs_minecraft_fixtures::fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a call through an alias of the crate",
                "use mcrs_minecraft_fixtures as fx;\n\n#[test]\nfn reads() {\n    let dump = fx::fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a call through a renamed import",
                "use mcrs_minecraft_fixtures::fixture as load;\n\n#[test]\nfn reads() {\n    let dump = load(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a call through a glob import",
                "use mcrs_minecraft_fixtures::*;\n\n#[test]\nfn reads() {\n    let dump = fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a call through a grouped import",
                "use mcrs_minecraft_fixtures::{fixture, Fixture};\n\n#[test]\nfn reads() {\n    let dump: Fixture = fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a call in a helper of the same file",
                "use mcrs_minecraft_fixtures::fixture;\n\nfn dump() -> Vec<u8> {\n    fixture(\"demo\").bytes()\n}\n\n#[test]\nfn reads() {\n    assert!(!dump().is_empty());\n}\n",
            ),
        ];
        for (label, source) in positives {
            assert_eq!(category_of(&loads(source)), Some("corpus"), "{label}");
        }
        let negatives = [
            (
                "the same function name from another crate",
                "use other::fixture;\n\n#[test]\nfn reads() {\n    let dump = fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a crate whose name only starts with the loader's",
                "use mcrs_minecraft_fixtures_extra::fixture;\n\n#[test]\nfn reads() {\n    let dump = fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a function of the same name declared in the file",
                "fn fixture(name: &str) -> Option<u8> {\n    None\n}\n\n#[test]\nfn reads() {\n    let dump = fixture(\"demo\");\n    assert!(dump.is_some());\n}\n",
            ),
            (
                "a method of the same name",
                "#[test]\nfn reads() {\n    let dump = store.fixture(\"demo\");\n    assert!(dump.is_ok());\n}\n",
            ),
            (
                "a glob import and a call to a name the loader does not export",
                "use mcrs_minecraft_fixtures::*;\n\n#[test]\nfn reads() {\n    let value = compute();\n    assert_eq!(value, 3);\n}\n",
            ),
            (
                "the crate named only in a string",
                "#[test]\nfn reads() {\n    let note = \"mcrs_minecraft_fixtures::fixture\";\n    assert!(!note.is_empty());\n}\n",
            ),
        ];
        for (label, source) in negatives {
            assert_eq!(category_of(&loads(source)), None, "{label}");
        }
    }

    #[test]
    fn a_determinism_match_needs_distinct_immutable_bindings_that_read_no_mutable_state() {
        let rows: [(&str, &'static str, &'static str, bool); 5] = [
            (
                "two calls with the same input",
                "#[test]\nfn t() {\n    let a = generate(7);\n    let b = generate(7);\n    assert_eq!(a, b);\n}\n",
                "t",
                true,
            ),
            (
                "one name bound twice and compared against expectations",
                "#[test]\nfn t() {\n    let random = Legacy::new(123);\n    assert_eq!(random.next(), 1);\n    let random = Legacy::new(123);\n    assert_eq!(random.next(), 2);\n}\n",
                "t",
                false,
            ),
            (
                "a generator replayed by hand and compared with the one the code advanced",
                "#[test]\nfn t() {\n    let mut rng = Random::new(5);\n    place(&mut rng);\n    let mut replay = Random::new(5);\n    replay.next();\n    assert_eq!(rng, replay);\n}\n",
                "t",
                false,
            ),
            (
                "two sequential draws from one stream",
                "#[test]\nfn t() {\n    let mut raw = Random::new(7);\n    let hi = raw.next();\n    let lo = raw.next();\n    assert_eq!(total(), hi + lo);\n}\n",
                "t",
                false,
            ),
            (
                "two calls that read one mutable binding",
                "#[test]\nfn t() {\n    let mut state = State::new();\n    let a = measure(&mut state);\n    let b = measure(&mut state);\n    assert_eq!(a, b);\n}\n",
                "t",
                false,
            ),
        ];
        for (name, source, test, protected) in rows {
            let fixture = lib(source, test);
            let found = category_of(&fixture);
            assert_eq!(found == Some("determinism"), protected, "{name}: {found:?}");
        }
    }

    #[test]
    fn a_module_path_alone_protects_no_test() {
        let rows = [Row {
            category: "census",
            reason: "a census counts what the layout holds",
            module: Some(CONFORMANCE),
            matches: size_pin,
        }];
        let fixture = lib(
            "mod conformance {\n    #[test]\n    fn pins() {\n        assert!(size_of::<A>() <= 8);\n    }\n\n    #[test]\n    fn counts_bytes() {\n        assert_eq!(heap_bytes(&table).len(), 3);\n    }\n}\n\nmod elsewhere {\n    #[test]\n    fn pins() {\n        assert!(size_of::<A>() <= 8);\n    }\n}\n",
            "conformance::pins",
        );
        let report = report_of(&fixture, &rows);
        let row = |name: &str| {
            let test = test_in(&report, ("pkg", name));
            (test.category, test.unclassified)
        };
        assert_eq!(row("conformance::pins"), (Some("census"), false));
        assert_eq!(row("conformance::counts_bytes"), (None, true));
        assert_eq!(row("elsewhere::pins"), (None, false));
    }

    #[test]
    fn a_test_the_default_filter_skips_is_not_measured_and_not_protected() {
        let fixture = lib(
            "mod exhaustive {\n    #[test]\n    fn sweep() {\n        assert!(size_of::<A>() <= 8);\n    }\n}\n",
            "exhaustive::sweep",
        );
        let report = report_of(&fixture, &ROWS);
        let test = test_in(&report, fixture.test);
        assert_eq!((test.measured, test.category), (false, None));

        let real = "[profile.default]\ndefault-filter = \"not test(/(^|::)(exhaustive|gpu|online|vanilla_server|cargo_shear|cargo_mutants)::/)\"\n";
        assert_eq!(
            not_measured_markers(real).unwrap(),
            [
                "exhaustive",
                "gpu",
                "online",
                "vanilla_server",
                "cargo_shear",
                "cargo_mutants"
            ]
        );
        let unreadable = not_measured_markers("[profile.default]\ndefault-filter = \"all()\"\n");
        assert!(unreadable.unwrap_err().contains("all()"));
    }

    #[test]
    fn doctests_count_only_runnable_fences() {
        let docs = concat!(
            "//! ```\n//! let crate_level = 1;\n//! ```\n",
            "/// ```\n/// let plain = 1;\n/// ```\npub fn plain() {}\n",
            "/// ```rust\n/// let tagged = 1;\n/// ```\npub fn tagged() {}\n",
            "/// ```should_panic\n/// panic!();\n/// ```\npub fn panics() {}\n",
            "/// ```ignore\n/// ignored();\n/// ```\npub fn ignored() {}\n",
            "/// ```no_run\n/// loops();\n/// ```\npub fn no_run() {}\n",
            "/// ```text\n/// not code\n/// ```\npub fn text() {}\n",
            "/// ```toml\n/// a = 1\n/// ```\npub fn other_language() {}\n",
            "/// ```compile_fail\n/// let a: u8 = \"\";\n/// ```\npub fn compile_fail() {}\n",
            "pub struct S {\n    /// ```\n    /// let field = 1;\n    /// ```\n    pub field: u8,\n}\n",
            "impl S {\n    /// ```\n    /// let method = 1;\n    /// ```\n    pub fn method(&self) {}\n}\n",
            "/// ```\n/// let module = 1;\n/// ```\nmod inline {\n    pub fn nothing() {}\n}\n",
        );
        let fixture = Fixture {
            files: vec![
                ("pkg/src/lib.rs", docs),
                (
                    "pkg/tests/it/main.rs",
                    "/// ```\n/// let in_a_test_target = 1;\n/// ```\nfn helper() {}\n",
                ),
            ],
            targets: vec![
                ("pkg", "pkg", "pkg/src/lib.rs", true),
                ("pkg", "pkg::it", "pkg/tests/it/main.rs", false),
            ],
            test: ("pkg", ""),
        };
        let report = report_of(&fixture, &ROWS);
        assert_eq!(report.doctests["pkg"], 7);
    }

    #[test]
    fn nextest_test_names_map_to_their_source_files() {
        let metadata = r#"{"workspace_root":"/w","packages":[{"name":"p","edition":"2024","manifest_path":"/w/p/Cargo.toml","dependencies":[],"targets":[
            {"kind":["lib"],"name":"p","src_path":"/w/p/src/lib.rs"},
            {"kind":["test"],"name":"it","src_path":"/w/p/tests/it/main.rs"},
            {"kind":["bin"],"name":"tool","src_path":"/w/p/src/bin/tool.rs"},
            {"kind":["example"],"name":"demo","src_path":"/w/p/examples/demo.rs"},
            {"kind":["custom-build"],"name":"build-script-build","src_path":"/w/p/build.rs"}]}]}"#;
        let workspace = Workspace::parse(metadata).unwrap();
        let found: Vec<(String, String, bool)> = target_specs(&workspace)
            .into_iter()
            .map(|t| (t.binary, t.root, t.lib))
            .collect();
        assert_eq!(
            found,
            [
                ("p".to_owned(), "p/src/lib.rs".to_owned(), true),
                ("p::it".to_owned(), "p/tests/it/main.rs".to_owned(), false),
                (
                    "p::bin/tool".to_owned(),
                    "p/src/bin/tool.rs".to_owned(),
                    false
                ),
            ]
        );

        let files = files_of(&[
            (
                "p/src/lib.rs",
                "#[test]\nfn top() {}\nmod a;\n#[path = \"elsewhere/b.rs\"]\nmod b;\nmod c { mod d; }\nmod e;\n",
            ),
            ("p/src/a.rs", "#[test]\nfn in_a() {}\nmod inner;\n"),
            ("p/src/a/inner.rs", "#[test]\nfn in_inner() {}\n"),
            ("p/src/elsewhere/b.rs", "#[test]\nfn in_b() {}\n"),
            ("p/src/c/d.rs", "#[test]\nfn in_d() {}\n"),
            ("p/src/e/mod.rs", "#[test]\nfn in_e() {}\n"),
            (
                "p/tests/it/main.rs",
                "#[test]\nfn root() {}\nmod aquifer;\n",
            ),
            ("p/tests/it/aquifer.rs", "#[test]\nfn in_aquifer() {}\n"),
            ("p/src/bin/tool.rs", "#[test]\nfn in_tool() {}\n"),
        ]);
        let report = classify(&files, &target_specs(&workspace), &[], &ROWS).unwrap();
        let mapped: Vec<(&str, &str, &str)> = report
            .tests
            .iter()
            .map(|t| (t.id.binary.as_str(), t.id.name.as_str(), t.file.as_str()))
            .collect();
        assert_eq!(
            mapped,
            [
                ("p", "a::in_a", "p/src/a.rs"),
                ("p", "a::inner::in_inner", "p/src/a/inner.rs"),
                ("p", "b::in_b", "p/src/elsewhere/b.rs"),
                ("p", "c::d::in_d", "p/src/c/d.rs"),
                ("p", "e::in_e", "p/src/e/mod.rs"),
                ("p", "top", "p/src/lib.rs"),
                ("p::bin/tool", "in_tool", "p/src/bin/tool.rs"),
                ("p::it", "aquifer::in_aquifer", "p/tests/it/aquifer.rs"),
                ("p::it", "root", "p/tests/it/main.rs"),
            ]
        );
    }
}
