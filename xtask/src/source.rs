use crate::git;
use crate::scope::{self, Head};
use proc_macro2::{Span, TokenStream, TokenTree};
use std::collections::BTreeMap;
use std::path::Path;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprLit, Lit, LitStr, Macro, Meta};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::Repo;
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
