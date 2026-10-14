use super::rules;
use crate::source::{self, is_test, line};
use std::collections::BTreeSet;
use syn::visit::{self, Visit};
use syn::{Expr, ExprLit, ItemFn, Lit, Macro};

const ANY_CASE_TERMS: [&str; 5] = ["vanilla", "mojang", "java", "the reference", "the game"];
const CONNECTION_STATE: &str = "the game state";
const EXACT_CASE_TERM: &str = "Codec";
const CLASS_NAMES: [&str; 15] = [
    "BeehiveBlockEntity",
    "CanyonWorldCarver",
    "CaveWorldCarver",
    "ConstantFunction",
    "DoublePlantBlock",
    "FallenTreeFeature",
    "GeodeFeature",
    "IntProviders",
    "MonsterRoomFeature",
    "OreFeature",
    "ServerLevel",
    "SimpleBlockFeature",
    "SpeleothemClusterFeature",
    "UniformInt",
    "VeryBiasedToBottomHeight",
];

pub struct Claim {
    pub line: usize,
    pub test: String,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

fn is_claim(text: &str) -> bool {
    let lower = text.to_lowercase().replace(CONNECTION_STATE, "");
    ANY_CASE_TERMS.iter().any(|term| lower.contains(term))
        || text.contains(EXACT_CASE_TERM)
        || CLASS_NAMES.iter().any(|name| has_word(text, name))
}

#[derive(Default)]
struct Messages(Vec<String>);

impl<'ast> Visit<'ast> for Messages {
    fn visit_macro(&mut self, mac: &'ast Macro) {
        let Some((args, operands)) = rules::assertion_arguments(mac) else {
            return;
        };
        for arg in args.iter().skip(operands) {
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(text),
                ..
            }) = arg
            {
                self.0.push(text.value());
            }
        }
    }
}

struct Test {
    name: String,
    line: usize,
    end: usize,
    texts: Vec<String>,
}

#[derive(Default)]
struct Tests(Vec<Test>);

impl<'ast> Visit<'ast> for Tests {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        if is_test(&item.attrs) {
            let name = item.sig.ident.to_string();
            let mut texts = vec![name.replace('_', " ")];
            texts.extend(item.attrs.iter().filter_map(source::doc_line));
            let mut messages = Messages::default();
            messages.visit_block(&item.block);
            texts.extend(messages.0);
            self.0.push(Test {
                name,
                line: line(item.sig.ident.span()),
                end: line(item.block.brace_token.span.close()),
                texts,
            });
        }
        visit::visit_item_fn(self, item);
    }
}

pub fn warnings(path: &str, file: &syn::File, loader_functions: &BTreeSet<String>) -> Vec<Claim> {
    if rules::is_parity_file(path) {
        return Vec::new();
    }
    let mut found = Tests::default();
    found.visit_file(file);
    let claimed: Vec<Test> = found
        .0
        .into_iter()
        .filter(|test| test.texts.iter().any(|text| is_claim(text)))
        .collect();
    if claimed.is_empty() {
        return Vec::new();
    }
    let loaderless = rules::loaderless_tests(file, loader_functions);
    let literal = rules::literal_assertions(file);
    claimed
        .into_iter()
        .filter(|test| loaderless.contains(&test.line))
        .filter(|test| literal.iter().any(|at| (test.line..=test.end).contains(at)))
        .map(|test| Claim {
            line: test.line,
            test: test.name,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "crates/demo/tests/it/checks.rs";

    fn claims_in(path: &str, source: &str, loader: &[&str]) -> Vec<(usize, String)> {
        let file = syn::parse_file(source).expect("test source parses");
        let loader = loader.iter().map(|n| (*n).to_owned()).collect();
        warnings(path, &file, &loader)
            .into_iter()
            .map(|claim| (claim.line, claim.test))
            .collect()
    }

    #[test]
    fn each_claim_term_raises_a_warning() {
        let terms = [
            "vanilla",
            "Vanilla",
            "VANILLA",
            "mojang",
            "Mojang",
            "java",
            "Java",
            "the reference",
            "The Reference",
            "the game",
            "The game",
            "Codec",
            "ServerLevel",
        ];
        for term in terms {
            let ident = term.replace(' ', "_");
            let rows = [
                (
                    "name",
                    format!("#[test]\nfn {ident}_holds() {{\n    assert_eq!(got(), 1);\n}}\n"),
                    (2, format!("{ident}_holds")),
                ),
                (
                    "doc",
                    format!(
                        "/// Compares {term} output.\n#[test]\nfn holds() {{\n    assert_eq!(got(), 1);\n}}\n"
                    ),
                    (3, "holds".to_owned()),
                ),
                (
                    "message of a comparison",
                    format!(
                        "#[test]\nfn holds() {{\n    assert_eq!(got(), 1, \"{term} says so\");\n}}\n"
                    ),
                    (2, "holds".to_owned()),
                ),
                (
                    "message of a condition",
                    format!(
                        "#[test]\nfn holds() {{\n    assert!(got() == 1, \"{term} says so\");\n}}\n"
                    ),
                    (2, "holds".to_owned()),
                ),
                (
                    "message after a format argument",
                    format!(
                        "#[test]\nfn holds() {{\n    assert_ne!(got(), 1, \"{{}} {term}\", other());\n}}\n"
                    ),
                    (2, "holds".to_owned()),
                ),
                (
                    "message of a debug assertion",
                    format!(
                        "#[test]\nfn holds() {{\n    debug_assert_eq!(got(), 1, \"{term}\");\n}}\n"
                    ),
                    (2, "holds".to_owned()),
                ),
            ];
            for (place, source, expected) in rows {
                assert_eq!(
                    claims_in(PATH, &source, &[]),
                    [expected],
                    "{term} in the {place}"
                );
            }
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of tests and files that must stay silent"
    )]
    fn tests_that_fail_a_warning_condition_stay_silent() {
        let loads = "use mcrs_minecraft_fixtures::fixture;\nfn load() {\n    fixture(\"x\");\n}\n";
        let rows: [(&str, &str, String, &[&str]); 17] = [
            (
                "no claim term",
                PATH,
                "#[test]\nfn holds() {\n    assert_eq!(got(), 1);\n}\n".to_owned(),
                &[],
            ),
            (
                "a claim with a loader call in a helper of the same file",
                PATH,
                format!(
                    "{loads}#[test]\nfn vanilla_value() {{\n    load();\n    assert_eq!(got(), 1);\n}}\n"
                ),
                &[],
            ),
            (
                "a claim with a loader call in the test itself",
                PATH,
                "use mcrs_minecraft_fixtures::fixture;\n#[test]\nfn vanilla_value() {\n    fixture(\"x\");\n    assert_eq!(got(), 1);\n}\n".to_owned(),
                &[],
            ),
            (
                "a claim imported through a glob of the loader",
                PATH,
                "use mcrs_minecraft_fixtures::*;\n#[test]\nfn vanilla_value() {\n    fixture(\"x\");\n    assert_eq!(got(), 1);\n}\n".to_owned(),
                &["fixture"],
            ),
            (
                "assertions compare no literal-derived operand",
                PATH,
                "#[test]\nfn vanilla_value() {\n    assert_eq!(got(), other());\n    assert!(got() < 3);\n}\n".to_owned(),
                &[],
            ),
            (
                "the term only in a compared literal",
                PATH,
                "#[test]\nfn holds() {\n    assert_eq!(got(), \"vanilla\");\n}\n".to_owned(),
                &[],
            ),
            (
                "the term only in a message of another macro",
                PATH,
                "#[test]\nfn holds() {\n    assert_eq!(got(), 1);\n    println!(\"vanilla\");\n}\n".to_owned(),
                &[],
            ),
            (
                "codec in lower case",
                PATH,
                "/// the codec\n#[test]\nfn codec_holds() {\n    assert_eq!(got(), 1, \"codec\");\n}\n".to_owned(),
                &[],
            ),
            (
                "a class name inside a longer word",
                PATH,
                "/// MyServerLevel and ServerLevelBuilder and ServerLevels\n#[test]\nfn holds() {\n    assert_eq!(got(), 1, \"x_ServerLevel ServerLevel_x ServerLevel2 2ServerLevel\");\n}\n".to_owned(),
                &[],
            ),
            (
                "a class name in a snake case name",
                PATH,
                "#[test]\nfn server_level_holds() {\n    assert_eq!(got(), 1);\n}\n".to_owned(),
                &[],
            ),
            (
                "a claim in a parity file",
                "crates/demo/tests/it/checks_parity.rs",
                "#[test]\nfn vanilla_value() {\n    assert_eq!(got(), 1);\n}\n".to_owned(),
                &[],
            ),
            (
                "a claim in a helper that is no test",
                PATH,
                "/// vanilla\nfn vanilla_helper() {\n    assert_eq!(got(), 1, \"vanilla\");\n}\n#[test]\nfn holds() {\n    helper();\n}\n".to_owned(),
                &[],
            ),
            (
                "the claim in one test and the literal in the next",
                PATH,
                "#[test]\nfn vanilla_value() {\n    assert_eq!(got(), other());\n}\n#[test]\nfn holds() {\n    assert_eq!(got(), 1);\n}\n".to_owned(),
                &[],
            ),
            (
                "the literal in one test and the claim in the next",
                PATH,
                "#[test]\nfn holds() {\n    assert_eq!(got(), 1);\n}\n#[test]\nfn vanilla_value() {\n    assert_eq!(got(), other());\n}\n".to_owned(),
                &[],
            ),
            (
                "the game state in a name",
                PATH,
                "#[test]\nfn reaches_the_game_state() {\n    assert_eq!(got(), 1);\n}\n".to_owned(),
                &[],
            ),
            (
                "the game state in a doc and a message",
                PATH,
                "/// Holds in The Game State.\n#[test]\nfn holds() {\n    assert_eq!(got(), 1, \"the game state\");\n}\n".to_owned(),
                &[],
            ),
            (
                "an empty file",
                PATH,
                String::new(),
                &[],
            ),
        ];
        for (label, path, source, loader) in rows {
            assert_eq!(claims_in(path, &source, loader), [], "{label}");
        }
    }

    #[test]
    fn a_claim_is_judged_per_test_and_located_at_the_test() {
        let source = concat!(
            "#[test]\n",
            "fn vanilla_unrelated() {\n",
            "    assert_eq!(got(), other());\n",
            "}\n",
            "\n",
            "#[test]\n",
            "fn compares() {\n",
            "    assert_eq!(got(), 1);\n",
            "}\n",
            "\n",
            "#[test]\n",
            "fn vanilla_value() {\n",
            "    let expected = 7;\n",
            "    assert_eq!(got(), expected);\n",
            "}\n",
            "\n",
            "#[test] fn java_one_liner() { assert_eq!(got(), 1); }\n",
            "\n",
            "#[cfg(test)]\n",
            "mod nested {\n",
            "    mod deeper {\n",
            "        #[test]\n",
            "        fn mojang_value() {\n",
            "            assert_eq!(got(), 1);\n",
            "        }\n",
            "    }\n",
            "}\n",
            "\n",
            "#[tokio::test]\n",
            "async fn the_reference_value() {\n",
            "    assert_eq!(got().await, 1);\n",
            "}\n",
            "\n",
            "#[test]\n",
            "fn the_game_state_and_the_game() {\n",
            "    assert_eq!(got(), 1);\n",
            "}\n",
            "\n",
            "const LIMIT: u32 = 3;\n",
            "\n",
            "#[test]\n",
            "fn vanilla_limit() {\n",
            "    assert_eq!(got(), LIMIT);\n",
            "}\n",
        );
        assert_eq!(
            claims_in(PATH, source, &[]),
            [
                (12, "vanilla_value".to_owned()),
                (17, "java_one_liner".to_owned()),
                (23, "mojang_value".to_owned()),
                (30, "the_reference_value".to_owned()),
                (35, "the_game_state_and_the_game".to_owned()),
                (42, "vanilla_limit".to_owned()),
            ]
        );
    }
}
