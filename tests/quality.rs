use std::path::{Path, PathBuf};

#[path = "quality/architecture.rs"]
mod architecture;
#[path = "quality/documentation.rs"]
mod documentation;
#[path = "quality/rust_references.rs"]
mod rust_references;

fn files(root: &Path) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if ["target", ".git", ".qualitygate", ".coverage"]
                .contains(&entry.file_name().to_string_lossy().as_ref())
            {
                continue;
            }
            visited += 1;
            assert!(visited <= 20_000, "Authored file inventory exceeds budget");
            assert!(
                !entry.file_type().unwrap().is_symlink(),
                "Symlink in authored files: {}",
                entry.path().display()
            );
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
fn documentation_authored_file_lengths_are_valid() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in files(root) {
        if file.file_name().unwrap() == "Cargo.lock" {
            continue;
        }
        assert!(
            std::fs::metadata(&file).unwrap().len() <= 2 * 1024 * 1024,
            "Authored file exceeds 2 MiB: {}",
            file.display()
        );
        let bytes = std::fs::read(&file).unwrap();
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        assert!(
            text.lines().count() <= 1000,
            "File exceeds 1000 lines: {}",
            file.display()
        );
    }
}

#[test]
fn architecture_authored_sources_are_rust_and_parseable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
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
