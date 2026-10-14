use crate::source::{file_name, is_test, line, walk_tokens};
use proc_macro2::{Delimiter, Group, Ident, TokenTree};
use std::collections::{BTreeSet, HashMap};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Arm, BinOp, Block, Expr, ExprClosure, ExprForLoop, ExprIf, ExprLet, ExprPath, ExprWhile, FnArg,
    ImplItemFn, ItemConst, ItemFn, ItemStatic, ItemUse, Macro, Pat, PatIdent, Signature,
    StaticMutability, Stmt, Token, UseTree,
};

pub const LOADS_NO_FIXTURE: &str = "loads no fixture: load the fixture through mcrs_minecraft_fixtures, or move this test out of the parity file";
pub const LITERAL_EXPECTED: &str = "literal expected value: compare against a value read from the fixture or from the code under test";

const LOADER_CRATE: &str = "mcrs_minecraft_fixtures";
const COMPARING_MACROS: [&str; 4] = [
    "assert_eq",
    "assert_ne",
    "debug_assert_eq",
    "debug_assert_ne",
];
const CONDITION_MACROS: [&str; 2] = ["assert", "debug_assert"];
const DEPTH: usize = 32;

pub fn is_parity_file(path: &str) -> bool {
    file_name(path)
        .strip_suffix(".rs")
        .is_some_and(|stem| stem == "parity" || stem.ends_with("_parity"))
}

pub fn check(file: &syn::File, loader_functions: &BTreeSet<String>) -> Vec<(usize, &'static str)> {
    let mut found = tests_without_a_loader(file, loader_functions);
    found.extend(literal_expectations(file));
    found
}

#[derive(Default)]
struct Idents(BTreeSet<String>);

impl<'ast> Visit<'ast> for Idents {
    fn visit_ident(&mut self, ident: &'ast Ident) {
        self.0.insert(ident.to_string());
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        walk_tokens(&mac.tokens, &mut |tokens, at| {
            if let TokenTree::Ident(ident) = &tokens[at] {
                self.0.insert(ident.to_string());
            }
        });
        visit::visit_macro(self, mac);
    }
}

fn idents_of(visit: impl FnOnce(&mut Idents)) -> BTreeSet<String> {
    let mut idents = Idents::default();
    visit(&mut idents);
    idents.0
}

#[derive(Default)]
struct Items {
    bodies: HashMap<String, BTreeSet<String>>,
    tests: Vec<(usize, BTreeSet<String>)>,
    imported: BTreeSet<String>,
    glob: bool,
}

impl Items {
    fn record(&mut self, name: &Ident, idents: BTreeSet<String>) {
        self.bodies
            .entry(name.to_string())
            .or_default()
            .extend(idents);
    }

    fn import(&mut self, tree: &UseTree, inside: bool) {
        match tree {
            UseTree::Path(path) => {
                let inside = inside || path.ident == LOADER_CRATE;
                if inside {
                    self.import(&path.tree, true);
                }
            }
            UseTree::Name(name) => {
                if inside && name.ident != "self" {
                    self.imported.insert(name.ident.to_string());
                }
            }
            UseTree::Rename(rename) => {
                if inside || rename.ident == LOADER_CRATE {
                    self.imported.insert(rename.rename.to_string());
                }
            }
            UseTree::Glob(_) => self.glob |= inside,
            UseTree::Group(group) => {
                for item in &group.items {
                    self.import(item, inside);
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for Items {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        let idents = idents_of(|found| found.visit_block(&item.block));
        if is_test(&item.attrs) {
            self.tests
                .push((line(item.sig.ident.span()), idents.clone()));
        }
        self.record(&item.sig.ident, idents);
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        let idents = idents_of(|found| found.visit_block(&item.block));
        self.record(&item.sig.ident, idents);
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_item_const(&mut self, item: &'ast ItemConst) {
        let idents = idents_of(|found| found.visit_expr(&item.expr));
        self.record(&item.ident, idents);
        visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast ItemStatic) {
        let idents = idents_of(|found| found.visit_expr(&item.expr));
        self.record(&item.ident, idents);
        visit::visit_item_static(self, item);
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        self.import(&item.tree, false);
    }
}

fn reaches_loader(start: &BTreeSet<String>, items: &Items, loader: &BTreeSet<String>) -> bool {
    let mut seen: BTreeSet<&String> = BTreeSet::new();
    let mut pending: Vec<&String> = start.iter().collect();
    while let Some(name) = pending.pop() {
        if loader.contains(name) {
            return true;
        }
        if seen.insert(name)
            && let Some(body) = items.bodies.get(name)
        {
            pending.extend(body);
        }
    }
    false
}

fn tests_without_a_loader(
    file: &syn::File,
    loader_functions: &BTreeSet<String>,
) -> Vec<(usize, &'static str)> {
    loaderless_tests(file, loader_functions)
        .into_iter()
        .map(|line| (line, LOADS_NO_FIXTURE))
        .collect()
}

pub(super) fn loaderless_tests(
    file: &syn::File,
    loader_functions: &BTreeSet<String>,
) -> Vec<usize> {
    let mut items = Items::default();
    items.visit_file(file);
    let mut loader = items.imported.clone();
    loader.insert(LOADER_CRATE.to_owned());
    if items.glob {
        loader.extend(loader_functions.iter().cloned());
    }
    items
        .tests
        .iter()
        .filter(|(_, idents)| !reaches_loader(idents, &items, &loader))
        .map(|(line, _)| *line)
        .collect()
}

#[derive(Default)]
struct Constants<'ast>(HashMap<String, Vec<&'ast Expr>>);

impl<'ast> Visit<'ast> for Constants<'ast> {
    fn visit_item_const(&mut self, item: &'ast ItemConst) {
        self.0
            .entry(item.ident.to_string())
            .or_default()
            .push(&item.expr);
        visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast ItemStatic) {
        if matches!(item.mutability, StaticMutability::None) {
            self.0
                .entry(item.ident.to_string())
                .or_default()
                .push(&item.expr);
        }
        visit::visit_item_static(self, item);
    }
}

type Scope = HashMap<String, bool>;

struct Env<'a, 'ast> {
    constants: &'a HashMap<String, Vec<&'ast Expr>>,
    scopes: &'a [Scope],
}

fn plain_name(path: &ExprPath) -> Option<String> {
    if path.qself.is_some() {
        return None;
    }
    path.path.get_ident().map(ToString::to_string)
}

fn vec_macro_items(mac: &Macro) -> Option<Expr> {
    let is_vec = mac.path.segments.last().is_some_and(|s| s.ident == "vec");
    let bracketed = TokenTree::Group(Group::new(Delimiter::Bracket, mac.tokens.clone()));
    if is_vec {
        syn::parse2(bracketed.into()).ok()
    } else {
        None
    }
}

fn derived(env: &Env, expr: &Expr, depth: usize) -> bool {
    if depth > DEPTH {
        return false;
    }
    let again = |part: &Expr| derived(env, part, depth + 1);
    match expr {
        Expr::Lit(_) => true,
        Expr::Paren(inner) => again(&inner.expr),
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_) | syn::UnOp::Not(_)) && again(&unary.expr)
        }
        Expr::Cast(cast) => again(&cast.expr),
        Expr::Reference(reference) => again(&reference.expr),
        Expr::Array(array) => !array.elems.is_empty() && array.elems.iter().all(again),
        Expr::Repeat(repeat) => again(&repeat.expr) && again(&repeat.len),
        Expr::Tuple(tuple) => !tuple.elems.is_empty() && tuple.elems.iter().all(again),
        Expr::Struct(structure) => {
            structure.rest.is_none()
                && !structure.fields.is_empty()
                && structure.fields.iter().all(|field| again(&field.expr))
        }
        Expr::Call(call) => {
            matches!(&*call.func, Expr::Path(p) if p.path.is_ident("Some") || p.path.is_ident("Ok"))
                && !call.args.is_empty()
                && call.args.iter().all(again)
        }
        Expr::Binary(binary) => {
            matches!(
                binary.op,
                BinOp::Add(_)
                    | BinOp::Sub(_)
                    | BinOp::Mul(_)
                    | BinOp::Div(_)
                    | BinOp::Rem(_)
                    | BinOp::BitXor(_)
                    | BinOp::BitAnd(_)
                    | BinOp::BitOr(_)
                    | BinOp::Shl(_)
                    | BinOp::Shr(_)
            ) && again(&binary.left)
                && again(&binary.right)
        }
        Expr::Index(index) => again(&index.expr),
        Expr::Field(field) => again(&field.base),
        Expr::Macro(mac) => vec_macro_items(&mac.mac).is_some_and(|items| again(&items)),
        Expr::Path(path) => {
            let Some(name) = plain_name(path) else {
                return false;
            };
            if let Some(bound) = env.scopes.iter().rev().find_map(|scope| scope.get(&name)) {
                return *bound;
            }
            let bare = Env {
                constants: env.constants,
                scopes: &[],
            };
            env.constants
                .get(&name)
                .is_some_and(|inits| inits.iter().all(|init| derived(&bare, init, depth + 1)))
        }
        _ => false,
    }
}

#[derive(Default)]
struct PatternNames(Vec<String>);

impl<'ast> Visit<'ast> for PatternNames {
    fn visit_pat_ident(&mut self, pat: &'ast PatIdent) {
        self.0.push(pat.ident.to_string());
        visit::visit_pat_ident(self, pat);
    }
}

pub(super) fn assertion_arguments(mac: &Macro) -> Option<(Vec<Expr>, usize)> {
    let name = mac.path.segments.last()?.ident.to_string();
    let operands = if COMPARING_MACROS.contains(&name.as_str()) {
        2
    } else if CONDITION_MACROS.contains(&name.as_str()) {
        1
    } else {
        return None;
    };
    let args = mac
        .parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
        .ok()?
        .into_iter()
        .collect();
    Some((args, operands))
}

fn compared(mac: &Macro) -> Option<(Expr, Expr)> {
    let (args, operands) = assertion_arguments(mac)?;
    let mut args = args.into_iter();
    if operands == 2 {
        return Some((args.next()?, args.next()?));
    }
    match args.next()? {
        Expr::Binary(binary) if matches!(binary.op, BinOp::Eq(_) | BinOp::Ne(_)) => {
            Some((*binary.left, *binary.right))
        }
        _ => None,
    }
}

struct Operands<'a, 'ast> {
    constants: &'a HashMap<String, Vec<&'ast Expr>>,
    scopes: Vec<Scope>,
    found: Vec<usize>,
}

impl Operands<'_, '_> {
    fn derived(&self, expr: &Expr) -> bool {
        let env = Env {
            constants: self.constants,
            scopes: &self.scopes,
        };
        derived(&env, expr, 0)
    }

    fn bind(&mut self, name: String, literal: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, literal);
        }
    }

    fn shadow(&mut self, pat: &Pat) {
        let mut names = PatternNames::default();
        names.visit_pat(pat);
        for name in names.0 {
            self.bind(name, false);
        }
    }

    fn bind_pattern(&mut self, pat: &Pat, init: Option<&Expr>) {
        match (pat, init) {
            (Pat::Type(typed), _) => self.bind_pattern(&typed.pat, init),
            (Pat::Ident(ident), Some(init))
                if ident.by_ref.is_none()
                    && ident.mutability.is_none()
                    && ident.subpat.is_none() =>
            {
                let literal = self.derived(init);
                self.bind(ident.ident.to_string(), literal);
            }
            (Pat::Tuple(pats), Some(Expr::Tuple(values)))
                if pats.elems.len() == values.elems.len() =>
            {
                for (pat, value) in pats.elems.iter().zip(&values.elems) {
                    self.bind_pattern(pat, Some(value));
                }
            }
            _ => self.shadow(pat),
        }
    }

    fn scoped(&mut self, inside: impl FnOnce(&mut Self)) {
        self.scopes.push(Scope::new());
        inside(self);
        self.scopes.pop();
    }
}

impl<'ast> Operands<'_, 'ast> {
    fn function(&mut self, sig: &'ast Signature, block: &'ast Block) {
        let outer = std::mem::replace(&mut self.scopes, vec![Scope::new()]);
        for input in &sig.inputs {
            if let FnArg::Typed(typed) = input {
                self.shadow(&typed.pat);
            }
        }
        self.visit_block(block);
        self.scopes = outer;
    }
}

impl<'ast> Visit<'ast> for Operands<'_, 'ast> {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.function(&item.sig, &item.block);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.function(&item.sig, &item.block);
    }

    fn visit_block(&mut self, block: &'ast Block) {
        self.scoped(|this| {
            for stmt in &block.stmts {
                let Stmt::Local(local) = stmt else {
                    this.visit_stmt(stmt);
                    continue;
                };
                if let Some(init) = &local.init {
                    this.visit_expr(&init.expr);
                    if let Some((_, diverge)) = &init.diverge {
                        this.visit_expr(diverge);
                    }
                }
                this.bind_pattern(&local.pat, local.init.as_ref().map(|init| &*init.expr));
            }
        });
    }

    fn visit_expr_closure(&mut self, closure: &'ast ExprClosure) {
        self.scoped(|this| {
            for input in &closure.inputs {
                this.shadow(input);
            }
            this.visit_expr(&closure.body);
        });
    }

    fn visit_expr_for_loop(&mut self, looped: &'ast ExprForLoop) {
        self.visit_expr(&looped.expr);
        self.scoped(|this| {
            this.shadow(&looped.pat);
            this.visit_block(&looped.body);
        });
    }

    fn visit_expr_if(&mut self, branch: &'ast ExprIf) {
        self.scoped(|this| {
            this.visit_expr(&branch.cond);
            this.visit_block(&branch.then_branch);
        });
        if let Some((_, otherwise)) = &branch.else_branch {
            self.visit_expr(otherwise);
        }
    }

    fn visit_expr_while(&mut self, looped: &'ast ExprWhile) {
        self.scoped(|this| {
            this.visit_expr(&looped.cond);
            this.visit_block(&looped.body);
        });
    }

    fn visit_expr_let(&mut self, binding: &'ast ExprLet) {
        self.visit_expr(&binding.expr);
        self.shadow(&binding.pat);
    }

    fn visit_arm(&mut self, arm: &'ast Arm) {
        self.scoped(|this| {
            this.shadow(&arm.pat);
            if let Some((_, guard)) = &arm.guard {
                this.visit_expr(guard);
            }
            this.visit_expr(&arm.body);
        });
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        let Some((left, right)) = compared(mac) else {
            return;
        };
        if (self.derived(&left) || self.derived(&right))
            && let Some(name) = mac.path.segments.last()
        {
            self.found.push(line(name.ident.span()));
        }
    }
}

fn literal_expectations(file: &syn::File) -> Vec<(usize, &'static str)> {
    literal_assertions(file)
        .into_iter()
        .map(|line| (line, LITERAL_EXPECTED))
        .collect()
}

pub(super) fn literal_assertions(file: &syn::File) -> Vec<usize> {
    let mut constants = Constants::default();
    constants.visit_file(file);
    let mut operands = Operands {
        constants: &constants.0,
        scopes: vec![Scope::new()],
        found: Vec::new(),
    };
    operands.visit_file(file);
    operands.found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_of(source: &str, loader_functions: &[&str]) -> Vec<&'static str> {
        let file = syn::parse_file(source).expect("test source parses");
        let names = loader_functions.iter().map(|n| (*n).to_owned()).collect();
        check(&file, &names)
            .into_iter()
            .map(|(_, rule)| rule)
            .collect()
    }

    fn body(statements: &str) -> String {
        format!("fn compare() {{ {statements} }}")
    }

    #[test]
    fn literal_expected_values_fail_in_a_parity_file() {
        let rows = [
            "assert_eq!(got(), 1);",
            "assert_eq!(1, got());",
            "assert_eq!(got(), \"minecraft:overworld\");",
            "assert_eq!(got(), -1);",
            "assert_eq!(got(), 1 as u8);",
            "assert_eq!(got(), [1, 2]);",
            "assert_eq!(got(), (1, 2));",
            "assert_eq!(got(), &1);",
            "assert_eq!(got(), vec![1, 2]);",
            "assert_eq!(got(), Some(1));",
            "assert_eq!(got(), Ok(1));",
            "assert_eq!(got(), Pair { a: 1, b: 2 });",
            "assert_eq!(got(), 2 * (3 + 4));",
            "assert_eq!(got(), [0; 4]);",
            "let expected = 5; assert_eq!(got(), expected);",
            "let (a, b) = (1, other()); assert_eq!(got(), a);",
            "let expected: u32 = 5; assert_eq!(got(), expected);",
            "assert!(got() == 1);",
            "assert!(1 != got());",
            "assert_ne!(got(), 1);",
            "debug_assert_eq!(got(), 1);",
            "debug_assert_ne!(got(), 1);",
            "debug_assert!(got() != 1);",
            "assert_eq!(got(), std::vec![1]);",
            "assert_eq!(got(), 1, \"message {}\", other());",
            "let check = || assert_eq!(got(), 1); check();",
            "for _ in other() { assert_eq!(got(), 1); }",
            "if other() { assert_eq!(got(), 1); }",
            "if other() {} else { assert_eq!(got(), 1); }",
            "if let Some(_) = other() { assert_eq!(got(), 1); }",
            "while other() { assert_eq!(got(), 1); }",
            "match other() { _ => assert_eq!(got(), 1), }",
            "let Some(_) = other() else { assert_eq!(got(), 1); return };",
            "let _ = { assert_eq!(got(), 1); 2 };",
        ];
        for row in rows {
            assert_eq!(
                rules_of(&body(row), &[]),
                [LITERAL_EXPECTED],
                "{row} must fail"
            );
        }
        let with_items = [
            "const EXPECTED: u32 = 5; fn compare() { assert_eq!(got(), EXPECTED); }",
            "static EXPECTED: u32 = 5; fn compare() { assert_eq!(got(), EXPECTED); }",
            "const TABLE: [u32; 2] = [1, 2]; fn compare() { assert_eq!(got(), TABLE[1]); }",
            "const PAIR: (u32, u32) = (1, 2); fn compare() { assert_eq!(got(), PAIR.0); }",
            "const A: u32 = 1; const B: u32 = A + 1; fn compare() { assert_eq!(got(), B); }",
            "fn compare() { let expected = [1, 2]; assert_eq!(got(), expected[0]); }",
            "fn compare() { let x = 1; { assert_eq!(got(), x); } }",
            "fn compare() { let x = other(); { let x = 1; assert_eq!(got(), x); } }",
            "mod inner { fn compare() { assert_eq!(got(), 1); } }",
            "impl Holder { fn compare(&self) { assert_eq!(got(), 1); } }",
            "fn compare() { let outer = 1; fn nested() { assert_eq!(got(), 2); } }",
        ];
        for source in with_items {
            assert_eq!(
                rules_of(source, &[]),
                [LITERAL_EXPECTED],
                "{source} must fail"
            );
        }
    }

    #[test]
    fn values_not_computed_from_literals_alone_pass() {
        let rows = [
            "let a = 1; let a = other(); assert_eq!(got(), a);",
            "let mut n = 0; n += 1; assert_eq!(got(), n);",
            "assert_eq!(got(), LIMIT);",
            "assert_eq!(got(), Kind::A);",
            "assert_eq!(got(), \"a\".to_owned());",
            "assert_eq!(got(), format!(\"x{}\", 1));",
            "assert_eq!(got(), rl!(\"minecraft:stone\"));",
            "assert_eq!(got(), []);",
            "assert_eq!(got(), vec![]);",
            "assert_eq!(got(), other(), \"message {}\", 1);",
            "assert!(got() < 3);",
            "assert!(got() == other(), \"message {}\", 1);",
            "assert!(got());",
            "assert_eq!(got(), other());",
            "assert_eq!(got(), Some(other()));",
            "assert_eq!(got(), Pair { a: 1, b: other() });",
            "assert_eq!(got(), Pair { a: 1, ..other() });",
            "assert_eq!(got(), 1 + other());",
            "assert_eq!(got(), 1 == 1);",
            "assert_eq!(got(), *other());",
            "assert_eq!(got(), *&1);",
            "assert_eq!(got(), -other());",
            "assert_eq!(got(), !other());",
            "assert_eq!(got(), [0; LEN]);",
            "assert_eq!(got(), [other(); 4]);",
            "let (a, b) = (other(), 1, 2); assert_eq!(got(), b);",
            "assert_eq!(got(), Err(1));",
            "assert_eq!(got(), ());",
            "assert_eq!(got(), Some());",
            "let n = 3; for n in other() { assert_eq!(got(), n); }",
            "let n = 3; match other() { n => assert_eq!(got(), n), }",
            "let n = 3; if let Some(n) = other() { assert_eq!(got(), n); }",
            "let n = 3; while let Some(n) = other() { assert_eq!(got(), n); }",
            "let n = 3; let check = |n| assert_eq!(got(), n); check(other());",
            "match other() { m if m == 1 => assert_eq!(got(), m), _ => {} }",
            "let (a, b) = (1, other()); assert_eq!(got(), b);",
            "let (a, b) = other(); assert_eq!(got(), a);",
            "let ref a = 1; assert_eq!(got(), a);",
            "let a @ 1 = 1 else { return }; assert_eq!(got(), a);",
            "let expected = 1; { let expected = other(); assert_eq!(got(), expected); }",
        ];
        for row in rows {
            assert_eq!(
                rules_of(&body(row), &[]),
                Vec::<&str>::new(),
                "{row} must pass"
            );
        }
        let with_items = [
            "const EXPECTED: u32 = 5; fn compare(EXPECTED: u32) { assert_eq!(got(), EXPECTED); }",
            "const EXPECTED: u32 = other(); fn compare() { assert_eq!(got(), EXPECTED); }",
            "static mut COUNT: u32 = 0; fn compare() { assert_eq!(got(), COUNT); }",
            "const A: u32 = B; const B: u32 = A; fn compare() { assert_eq!(got(), A); }",
            "fn compare() { let x = 1; fn nested() { assert_eq!(got(), x); } }",
        ];
        for source in with_items {
            assert_eq!(
                rules_of(source, &[]),
                Vec::<&str>::new(),
                "{source} must pass"
            );
        }
    }

    #[test]
    fn an_expression_nested_past_the_bound_is_no_longer_followed() {
        let nested = |depth: usize| {
            body(&format!(
                "assert_eq!(got(), {}1{});",
                "(".repeat(depth),
                ")".repeat(depth)
            ))
        };
        assert_eq!(rules_of(&nested(32), &[]), [LITERAL_EXPECTED]);
        assert_eq!(rules_of(&nested(33), &[]), Vec::<&str>::new());
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of sources that do or do not reach the loader within their file"
    )]
    fn a_parity_test_must_reach_the_loader_within_its_file() {
        let loads: [(&str, &[&str]); 14] = [
            (
                "use mcrs_minecraft_fixtures::fixture; const BYTES: u8 = fixture(\"x\"); #[test] fn t() { BYTES; }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::fixture; impl Holder { fn load() { fixture(\"x\"); } } #[test] fn t() { Holder::load(); }",
                &[],
            ),
            (
                "#[test] fn t() { mcrs_minecraft_fixtures::fixture(\"x\"); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::fixture; #[test] fn t() { fixture(\"x\"); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::{fixture, Fixture}; #[test] fn t() { fixture(\"x\"); }",
                &[],
            ),
            (
                "use ::mcrs_minecraft_fixtures::fixture; #[test] fn t() { fixture(\"x\"); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::fixture as load; #[test] fn t() { load(\"x\"); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures as fx; #[test] fn t() { fx::fixture(\"x\"); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::{self as fx}; #[test] fn t() { fx::fixture(\"x\"); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::*; #[test] fn t() { fixture(\"x\"); }",
                &["fixture"],
            ),
            (
                "use mcrs_minecraft_fixtures::fixture; fn a() { b() } fn b() { c() } fn c() { fixture(\"x\"); } #[test] fn t() { a(); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::fixture; static F: LazyLock<Vec<u8>> = LazyLock::new(|| fixture(\"x\").bytes()); #[test] fn t() { F.len(); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::fixture; #[test] fn t() { assert_eq!(fixture(\"x\").text(), other()); }",
                &[],
            ),
            (
                "mod inner { #[test] fn t() { use mcrs_minecraft_fixtures::fixture; fixture(\"x\"); } }",
                &[],
            ),
        ];
        for (source, loader) in loads {
            assert_eq!(
                rules_of(source, loader),
                Vec::<&str>::new(),
                "{source} must pass"
            );
        }
        let refused: [(&str, &[&str]); 10] = [
            ("use fixture; #[test] fn t() { fixture(\"x\"); }", &[]),
            ("#[test] fn t() { other(); }", &[]),
            ("use crate::support::read; #[test] fn t() { read(); }", &[]),
            (
                "use mcrs_minecraft_fixtures::*; #[test] fn t() { fixture(\"x\"); }",
                &[],
            ),
            (
                "use mcrs_minecraft_fixtures::fixture; fn helper() { fixture(\"x\"); } #[test] fn t() { other(); }",
                &[],
            ),
            ("fn a() { b() } fn b() { a() } #[test] fn t() { a(); }", &[]),
            ("mod inner { #[test] fn t() { other(); } }", &[]),
            ("#[tokio::test] async fn t() { other().await; }", &[]),
            (
                "#[cfg(test)] mod tests { #[test] fn t() { other(); } }",
                &[],
            ),
            (
                "use other::fixture; #[test] fn t() { fixture(\"x\"); }",
                &[],
            ),
        ];
        for (source, loader) in refused {
            assert_eq!(
                rules_of(source, loader),
                [LOADS_NO_FIXTURE],
                "{source} must fail"
            );
        }
        assert_eq!(
            rules_of("fn helper() { other(); }", &[]),
            Vec::<&str>::new(),
            "only tests must load a fixture"
        );
    }

    #[test]
    fn only_parity_files_are_held_to_the_parity_rules() {
        let rows = [
            ("crates/a/tests/it/x_parity.rs", true),
            ("crates/a/tests/it/parity.rs", true),
            ("x_parity.rs", true),
            ("crates/a/tests/it/parity_x.rs", false),
            ("crates/a/tests/it/parity/mod.rs", false),
            ("crates/a/tests/it/capture.rs", false),
            ("crates/a/tests/it/x_parity.txt", false),
            ("crates/a/tests/it/xparity.rs", false),
        ];
        for (path, expected) in rows {
            assert_eq!(is_parity_file(path), expected, "{path}");
        }
    }
}
