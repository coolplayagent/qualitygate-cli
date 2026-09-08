use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

fn files(root: &Path) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if ["target", ".git", ".qualitygate", ".coverage"]
                .contains(&entry.file_name().to_string_lossy().as_ref())
            {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else {
                entries.push(entry.path());
            }
        }
    }
    entries
}

#[test]
fn documentation_links_fences_and_authored_file_lengths_are_valid() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let link = regex::Regex::new(r"\]\(([^)]+)\)").unwrap();
    for file in files(root) {
        if file.file_name().unwrap() == "Cargo.lock" {
            continue;
        }
        let bytes = std::fs::read(&file).unwrap();
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        assert!(
            text.lines().count() <= 1000,
            "File exceeds 1000 lines: {}",
            file.display()
        );
        if file.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        assert_eq!(
            text.lines().filter(|line| line.starts_with("```")).count() % 2,
            0,
            "Unbalanced fence: {}",
            file.display()
        );
        for capture in link.captures_iter(text) {
            let target = &capture[1];
            if target.contains("://") || target.starts_with('#') {
                continue;
            }
            let target = target.split('#').next().unwrap();
            assert!(
                file.parent().unwrap().join(target).exists(),
                "Broken link in {}: {target}",
                file.display()
            );
        }
    }
}

#[test]
fn architecture_keeps_domain_pure_and_implementation_rust_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let prohibited: BTreeSet<_> = ["process", "fs", "net", "env"].into_iter().collect();
    for file in files(&root.join("src")) {
        assert_eq!(
            file.extension().unwrap(),
            "rs",
            "Non-Rust implementation: {}",
            file.display()
        );
        let text = std::fs::read_to_string(&file).unwrap();
        syn::parse_file(&text).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        assert!(!text.contains("#[allow(dead_code)]"));
        if file.components().any(|part| part.as_os_str() == "domain") {
            for namespace in &prohibited {
                assert!(
                    !text.contains(&format!("std::{namespace}")),
                    "Domain owns external I/O: {}",
                    file.display()
                );
            }
        }
        if file.file_name().unwrap() != "env.rs"
            && file.file_name().unwrap() != "main.rs"
            && !file.file_name().unwrap().to_string_lossy().contains("test")
        {
            assert!(
                !text.contains("std::env::"),
                "Environment access outside env: {}",
                file.display()
            );
        }
    }
}

#[test]
fn documentation_yaml_contracts_are_parseable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in files(root).into_iter().filter(|file| {
        file.extension()
            .is_some_and(|extension| extension == "yaml" || extension == "yml")
    }) {
        let text = std::fs::read_to_string(&file).unwrap();
        let value: serde_norway::Value = serde_norway::from_str(&text)
            .unwrap_or_else(|error| panic!("Invalid YAML {}: {error}", file.display()));
        assert!(
            value.is_mapping(),
            "Expected YAML configuration map: {}",
            file.display()
        );
    }
}
