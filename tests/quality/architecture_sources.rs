//! Find source files outside declared production or test module trees.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

fn children(
    items: &[syn::Item],
    file_directory: &Path,
    module_directory: &Path,
    pending: &mut Vec<PathBuf>,
) {
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let explicit = module.attrs.iter().find_map(|attribute| {
            if !attribute.path().is_ident("path") {
                return None;
            }
            let syn::Meta::NameValue(value) = &attribute.meta else {
                panic!("Unsupported module path attribute")
            };
            let syn::Expr::Lit(value) = &value.value else {
                panic!("Module path must be literal")
            };
            let syn::Lit::Str(value) = &value.lit else {
                panic!("Module path must be text")
            };
            Some(file_directory.join(value.value()))
        });
        if let Some((_, items)) = &module.content {
            let directory =
                explicit.unwrap_or_else(|| module_directory.join(module.ident.to_string()));
            children(items, &directory, &directory, pending);
        } else if let Some(path) = explicit {
            pending.push(path);
        } else {
            let base = module_directory.join(module.ident.to_string());
            let flat = base.with_extension("rs");
            let nested = base.join("mod.rs");
            assert_ne!(
                flat.exists(),
                nested.exists(),
                "Missing or ambiguous source module: {}",
                base.display()
            );
            pending.push(if flat.exists() { flat } else { nested });
        }
    }
}

fn reachable(root: &Path, mut pending: Vec<PathBuf>) -> BTreeSet<PathBuf> {
    let root = root.canonicalize().unwrap();
    let mut seen = BTreeSet::new();
    while let Some(file) = pending.pop() {
        let file = file.canonicalize().unwrap();
        assert!(
            file.starts_with(&root),
            "Source escapes repository: {}",
            file.display()
        );
        if !seen.insert(file.clone()) {
            continue;
        }
        assert!(
            seen.len() <= 4096,
            "Declared source inventory exceeds budget"
        );
        let text = std::fs::read_to_string(&file).unwrap();
        let syntax = syn::parse_file(&text).unwrap();
        let parent = file.parent().unwrap();
        let module_directory = if matches!(
            file.file_name().and_then(|name| name.to_str()),
            Some("lib.rs" | "main.rs" | "mod.rs")
        ) {
            parent.to_path_buf()
        } else {
            file.with_extension("")
        };
        children(&syntax.items, parent, &module_directory, &mut pending);
    }
    seen
}

pub(super) fn assert_complete(root: &Path, entries: &[(String, String, String)]) {
    let seen = reachable(
        root,
        entries.iter().map(|(_, _, path)| root.join(path)).collect(),
    );
    let bazel_roots: BTreeSet<_> = [
        "src/qualitygate/bazel_lib.rs",
        "src/qualitygate/net/bazel_lib.rs",
        "src/qualitygate/runner/bazel_lib.rs",
        "src/qualitygate/snapshot/bazel_lib.rs",
    ]
    .into_iter()
    .map(|path| root.join(path).canonicalize().unwrap())
    .collect();
    let unowned: Vec<_> = super::super::files(&root.join("src"))
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| path.canonicalize().unwrap())
        .filter(|path| !seen.contains(path) && !bazel_roots.contains(path))
        .collect();
    assert!(
        unowned.is_empty(),
        "Source files unreachable from registered production/test roots: {unowned:?}"
    );
}

#[test]
fn architecture_source_inventory_follows_test_paths_without_exempting_test_named_orphans() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(
        root.join("lib.rs"),
        "#[cfg(test)] #[path=\"fixture.rs\"] mod tests;",
    )
    .unwrap();
    std::fs::write(root.join("fixture.rs"), "fn fixture() {} ").unwrap();
    std::fs::write(root.join("orphan_tests.rs"), "fn hidden() {} ").unwrap();
    let sources = reachable(root, vec![root.join("lib.rs")]);
    assert!(sources.contains(&root.join("fixture.rs").canonicalize().unwrap()));
    assert!(!sources.contains(&root.join("orphan_tests.rs").canonicalize().unwrap()));
}
