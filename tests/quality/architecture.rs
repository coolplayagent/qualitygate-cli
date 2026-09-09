use super::rust_references::{References, test_only};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const OWNERS: &[(&str, &[&str])] = &[
    ("domain", &[]),
    ("env", &[]),
    ("paths", &["env"]),
    ("net", &["domain", "env"]),
    ("runner", &["domain", "env"]),
    ("snapshot", &["domain", "net", "paths", "runner"]),
    ("config", &["domain", "paths"]),
    ("adapters", &["config", "domain", "paths", "snapshot"]),
    (
        "application",
        &[
            "adapters", "config", "domain", "env", "net", "paths", "runner", "snapshot",
        ],
    ),
    (
        "interfaces",
        &["application", "config", "domain", "snapshot"],
    ),
    ("main", &["interfaces"]),
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
struct Edge {
    from: String,
    to: String,
    file: String,
    line: usize,
}

#[derive(Default)]
struct Graph {
    edges: BTreeSet<Edge>,
    errors: Vec<String>,
    files: BTreeSet<PathBuf>,
    sources: BTreeMap<String, String>,
}

impl Graph {
    fn source(&mut self, file: &str, location: Vec<String>, text: &str) -> References {
        let parsed = syn::parse_file(text).unwrap_or_else(|error| panic!("{file}: {error}"));
        let references = References::analyze(&parsed.items, location.clone());
        self.errors.extend(
            references
                .errors
                .iter()
                .map(|error| format!("{file}: {error}")),
        );
        let owner = location.first().map(String::as_str).unwrap_or("lib");
        for reference in &references.paths {
            let path = &reference.path;
            if path.first().is_some_and(|part| part == "crate") && path.len() > 1 {
                let target = &path[1];
                if target == "*" || !OWNERS.iter().any(|(name, _)| *name == target) {
                    self.errors.push(format!(
                        "{file}:{}: unknown root dependency {}",
                        reference.line,
                        path.join("::")
                    ));
                } else if owner != target {
                    self.edges.insert(Edge {
                        from: owner.into(),
                        to: target.clone(),
                        file: file.into(),
                        line: reference.line,
                    });
                }
            }
            self.capability(owner, file, reference.line, path);
        }
        references
    }

    fn capability(&mut self, owner: &str, file: &str, line: usize, path: &[String]) {
        let parts: Vec<_> = path.iter().map(String::as_str).collect();
        let external_io = matches!(
            parts.as_slice(),
            ["std" | "tokio", "fs" | "net" | "process" | "env" | "io", ..]
        ) || matches!(
            parts.first(),
            Some(&"reqwest" | &"process_wrap" | &"tempfile" | &"which" | &"file_id")
        );
        if owner == "domain" && external_io {
            self.errors.push(format!(
                "{file}:{line}: domain owns external I/O via {}",
                path.join("::")
            ));
        }
        if owner == "interfaces" && external_io {
            self.errors.push(format!(
                "{file}:{line}: interfaces must delegate external I/O via {}",
                path.join("::")
            ));
        }
        if owner == "domain" && matches!(parts.as_slice(), ["std" | "tokio"]) {
            self.errors.push(format!(
                "{file}:{line}: domain must import explicit pure capabilities"
            ));
        }
        if owner != "env" && matches!(parts.as_slice(), ["std", "env", ..]) {
            self.errors.push(format!(
                "{file}:{line}: environment access outside env via {}",
                path.join("::")
            ));
        }
        // Importing entire external namespaces through a glob cannot establish
        // which capability a later unqualified name uses.
        if matches!(parts.as_slice(), ["std" | "tokio", "*"]) {
            self.errors.push(format!(
                "{file}:{line}: external root glob hides capability ownership"
            ));
        }
    }

    fn load(&mut self, root: &Path, file: PathBuf, location: Vec<String>) {
        assert!(
            self.files.len() < 4096,
            "architecture source inventory exceeds budget"
        );
        assert!(
            self.files.insert(file.clone()),
            "duplicate source ownership: {}",
            file.display()
        );
        let metadata = std::fs::symlink_metadata(&file).unwrap();
        assert!(
            metadata.is_file() && metadata.len() <= 2 * 1024 * 1024,
            "invalid source: {}",
            file.display()
        );
        let name = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&file).unwrap();
        self.sources.insert(
            name.clone(),
            format!("sha256:{:x}", Sha256::digest(text.as_bytes())),
        );
        if location.is_empty() {
            let parsed = syn::parse_file(&text).unwrap();
            let declared: BTreeSet<_> = parsed
                .items
                .iter()
                .filter_map(|item| match item {
                    syn::Item::Mod(item) if !test_only(&item.attrs) && item.content.is_none() => {
                        Some(item.ident.to_string())
                    }
                    _ => None,
                })
                .collect();
            let expected: BTreeSet<_> = OWNERS
                .iter()
                .filter(|(name, _)| *name != "main")
                .map(|(name, _)| name.to_string())
                .collect();
            assert_eq!(
                declared, expected,
                "lib.rs must declare every reviewed owner"
            );
            assert!(
                parsed
                    .items
                    .iter()
                    .all(|item| matches!(item, syn::Item::Mod(_))),
                "crate root must only declare owners, without hidden root re-exports"
            );
        }
        let references = self.source(&name, location.clone(), &text);
        let directory = if matches!(
            file.file_name().and_then(|name| name.to_str()),
            Some("lib.rs" | "main.rs" | "mod.rs")
        ) {
            file.parent().unwrap().to_path_buf()
        } else {
            file.with_extension("")
        };
        for (child, _) in references.modules {
            let relative: PathBuf = child.iter().skip(location.len()).collect();
            let base = directory.join(relative);
            let flat = base.with_extension("rs");
            let nested = base.join("mod.rs");
            assert_ne!(
                flat.exists(),
                nested.exists(),
                "missing or ambiguous owner: {}",
                base.display()
            );
            self.load(root, if flat.exists() { flat } else { nested }, child);
        }
    }

    fn violations(&self) -> Vec<String> {
        let mut errors = self.errors.clone();
        for edge in &self.edges {
            let allowed = OWNERS
                .iter()
                .find(|(name, _)| *name == edge.from)
                .map(|(_, allowed)| *allowed)
                .unwrap_or_default();
            if !allowed.contains(&edge.to.as_str()) {
                errors.push(format!(
                    "{}:{}: forbidden dependency {} -> {}",
                    edge.file, edge.line, edge.from, edge.to
                ));
            }
        }
        if let Some(cycle) = self.cycle() {
            errors.push(format!("module dependency cycle: {}", cycle.join(" -> ")));
        }
        errors.sort();
        errors.dedup();
        errors
    }

    fn cycle(&self) -> Option<Vec<String>> {
        fn visit(
            node: &str,
            adjacency: &BTreeMap<&str, BTreeSet<&str>>,
            stack: &mut Vec<String>,
            done: &mut BTreeSet<String>,
        ) -> Option<Vec<String>> {
            if let Some(index) = stack.iter().position(|part| part == node) {
                let mut cycle = stack[index..].to_vec();
                cycle.push(node.into());
                return Some(cycle);
            }
            if done.contains(node) {
                return None;
            }
            stack.push(node.into());
            for next in adjacency.get(node).into_iter().flatten() {
                if let Some(cycle) = visit(next, adjacency, stack, done) {
                    return Some(cycle);
                }
            }
            stack.pop();
            done.insert(node.into());
            None
        }
        let mut adjacency = BTreeMap::<&str, BTreeSet<&str>>::new();
        for edge in &self.edges {
            adjacency.entry(&edge.from).or_default().insert(&edge.to);
        }
        let mut done = BTreeSet::new();
        for node in adjacency.keys() {
            if let Some(cycle) = visit(node, &adjacency, &mut Vec::new(), &mut done) {
                return Some(cycle);
            }
        }
        None
    }
}

#[test]
fn architecture_module_graph_is_acyclic_and_respects_owners() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut graph = Graph::default();
    graph.load(root, root.join("src/qualitygate/lib.rs"), vec![]);
    graph.load(
        root,
        root.join("src/qualitygate/main.rs"),
        vec!["main".into()],
    );
    let errors = graph.violations();
    let output = root.join("target/architecture");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 1, "scope": "production top-level Rust owners; all target/feature configurations",
        "files": graph.files.len(), "sources": graph.sources, "edges": graph.edges, "violations": errors,
    })).unwrap()).unwrap();
    let edges: BTreeSet<_> = graph
        .edges
        .iter()
        .map(|edge| format!("  \"{}\" -> \"{}\";", edge.from, edge.to))
        .collect();
    std::fs::write(
        output.join("graph.dot"),
        format!(
            "digraph qualitygate {{\n{}\n}}\n",
            edges.into_iter().collect::<Vec<_>>().join("\n")
        ),
    )
    .unwrap();
    assert!(
        errors.is_empty(),
        "{}\nSee target/architecture/report.json",
        errors.join("\n")
    );
}

#[test]
fn architecture_detects_grouped_alias_relative_macro_and_conditional_edges() {
    let mut graph = Graph::default();
    graph.source(
        "domain/sample.rs",
        vec!["domain".into(), "sample".into()],
        r#"
        use crate::{interfaces as ui, application::{self, CheckOptions}};
        pub use super::super::runner::capture;
        #[cfg(test)] fn test_only() { crate::adapters::run(); }
        #[cfg(not(test))] fn production() { crate::config::read(); }
        #[cfg(any(test, unix))] fn also_production() { crate::net::run(); }
        fn inside_macro() { assert!(crate::snapshot::run()); }
        mod nested { fn run() { super::super::super::paths::run(); } }
        // crate::env::must_not_be_a_dependency();
        const TEXT: &str = "crate::env::must_not_be_a_dependency()";
    "#,
    );
    let targets: BTreeSet<_> = graph.edges.iter().map(|edge| edge.to.as_str()).collect();
    assert_eq!(
        targets,
        BTreeSet::from([
            "interfaces",
            "application",
            "runner",
            "config",
            "net",
            "snapshot",
            "paths"
        ])
    );
    assert!(graph.edges.iter().all(|edge| edge.line > 1));
    assert_eq!(graph.violations().len(), graph.edges.len());
}

#[test]
fn architecture_rejects_cycles_and_capability_alias_bypasses() {
    let mut graph = Graph::default();
    graph.source(
        "domain/model.rs",
        vec!["domain".into()],
        r#"
        use std as sys;
        use tokio::{fs as disk};
        fn run() { sys::env::var("SECRET"); disk::read("file"); }
        pub use crate::interfaces::Cli;
    "#,
    );
    graph.source(
        "interfaces/mod.rs",
        vec!["interfaces".into()],
        "use crate::domain::Model;",
    );
    let errors = graph.violations().join("\n");
    assert!(errors.contains("domain owns external I/O"));
    assert!(errors.contains("environment access outside env"));
    assert!(errors.contains("domain -> interfaces -> domain"));
    assert!(errors.contains("forbidden dependency domain -> interfaces"));
    let parsed = References::analyze(
        &syn::parse_file(
            r#"
        #[cfg(test)] #[path = "external.rs"] mod tests;
        #[path = "external.rs"] mod hidden;
        include!("generated.rs");
        extern crate qualitygate as bypass;
    "#,
        )
        .unwrap()
        .items,
        vec!["domain".into()],
    );
    assert_eq!(parsed.errors.len(), 3);
    assert!(parsed.modules.is_empty());
}

#[test]
fn architecture_follows_declared_production_files_even_with_test_names() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(
        root.join("domain.rs"),
        "mod hidden_tests;\n#[cfg(test)] mod unavailable;\n",
    )
    .unwrap();
    std::fs::create_dir(root.join("domain")).unwrap();
    std::fs::write(
        root.join("domain/hidden_tests.rs"),
        "use crate::interfaces::Cli;",
    )
    .unwrap();
    let mut graph = Graph::default();
    graph.load(root, root.join("domain.rs"), vec!["domain".into()]);
    assert_eq!(graph.files.len(), 2);
    assert_eq!(graph.sources.len(), 2);
    assert!(
        graph
            .violations()
            .iter()
            .any(|error| error.contains("domain/hidden_tests.rs:1"))
    );
    let mut macros = Graph::default();
    macros.source(
        "domain.rs",
        vec!["domain".into()],
        "wrapper! { use crate::{interfaces::Cli, runner as run}; include!(\"hidden.rs\"); }",
    );
    assert!(macros.edges.iter().any(|edge| edge.to == "interfaces"));
    assert!(macros.edges.iter().any(|edge| edge.to == "runner"));
    assert!(
        macros
            .violations()
            .iter()
            .any(|error| error.contains("generated production source inside macro"))
    );
}
