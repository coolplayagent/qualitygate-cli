//! Bounded immutable old-code/new-test composition. Never edits a Git worktree.

use super::{File, MAX_FILES, Snapshot, content_digest_until};
use anyhow::{Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

pub struct Prepared {
    pub source_changed: bool,
    pub tests: BTreeSet<String>,
    pub overlay: BTreeMap<String, Option<String>>,
    pub baseline: Snapshot,
}

fn globs(patterns: &[String]) -> Result<globset::GlobSet> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(globset::Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}

pub fn prepare(
    snapshot: &Snapshot,
    source_paths: &[String],
    test_paths: &[String],
    support_paths: &[String],
    protected_paths: &[String],
    max_bytes: usize,
    max_file_bytes: usize,
) -> Result<Prepared> {
    if !(1..=crate::domain::snapshot_budget::MAX_FILE_BYTES).contains(&max_file_bytes) {
        bail!("Invalid bounded test overlay per-file budget");
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    let sources = globs(source_paths)?;
    let tests = globs(test_paths)?;
    let support = globs(support_paths)?;
    let protected = globs(protected_paths)?;
    let paths: BTreeSet<_> = snapshot
        .files
        .keys()
        .chain(snapshot.base_files.keys())
        .collect();
    let mut source_count = 0usize;
    let mut source_changed = false;
    let mut selected = BTreeSet::new();
    let mut overlay = BTreeMap::new();
    let mut files = snapshot.base_files.clone();
    for path in paths {
        super::changes::check_deadline(Some(deadline))?;
        let current = snapshot.files.get(path);
        let old = snapshot.base_files.get(path);
        let differs = current.map(|file| (&file.bytes, file.executable))
            != old.map(|file| (&file.bytes, file.executable));
        let is_test = tests.is_match(path);
        let is_overlay = is_test || support.is_match(path);
        if sources.is_match(path) {
            if is_overlay {
                bail!("Production and test/support paths overlap: {path}");
            }
            if snapshot.includes(path) {
                source_count += 1;
                source_changed |= differs;
            }
        }
        if !is_overlay {
            continue;
        }
        if differs && (protected.is_match(path) || build_input(path)) {
            bail!("Test overlay would replace a protected policy or build input: {path}");
        }
        if is_test
            && current.is_some()
            && snapshot.includes(path)
            && snapshot
                .changes
                .get(path)
                .is_some_and(|change| change.kind != "renamed")
            && current.map(|file| &file.bytes) != old.map(|file| &file.bytes)
        {
            selected.insert(path.clone());
        }
        if !differs {
            continue;
        }
        overlay.insert(path.clone(), current.map(|file| super::digest(&file.bytes)));
        match current {
            Some(file) => {
                files.insert(path.clone(), file.clone());
            }
            None => {
                files.remove(path);
            }
        }
    }
    if source_count == 0 {
        bail!("Test effectiveness source paths matched no captured files in scope");
    }
    let bytes = files.values().try_fold(0usize, |total, file: &File| {
        if file.bytes.len() > max_file_bytes {
            bail!("Test overlay file exceeds size budget");
        }
        total
            .checked_add(file.bytes.len())
            .ok_or_else(|| anyhow::anyhow!("Test overlay size overflow"))
    })?;
    if files.len() > MAX_FILES || bytes > max_bytes {
        bail!("Test overlay exceeds snapshot file or byte budget");
    }
    let mut identity = snapshot.identity.clone();
    identity.mode = "test_overlay".into();
    identity.head = identity.base.clone();
    identity.merge_request = None;
    super::bind_derived(&mut identity, content_digest_until(&files, Some(deadline))?);
    Ok(Prepared {
        source_changed,
        tests: selected,
        overlay,
        baseline: Snapshot {
            scope_evidence: snapshot.scope_evidence.clone(),
            root: snapshot.root.clone(),
            identity,
            files,
            base_files: BTreeMap::new(),
            changes: BTreeMap::new(),
            path_filter: snapshot.path_filter.clone(),
            commits: vec![],
        },
    })
}

fn build_input(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain",
        "rust-toolchain.toml",
        "build.rs",
        "pom.xml",
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "requirements.txt",
        "poetry.lock",
        "uv.lock",
        "Pipfile",
        "Pipfile.lock",
        "package.json",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "go.mod",
        "go.sum",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
        "gradle.lockfile",
        "Makefile",
        "CMakeLists.txt",
        "BUILD",
        "BUILD.bazel",
        "MODULE.bazel",
        "MODULE.bazel.lock",
        "WORKSPACE",
    ]
    .contains(&name)
        || name.ends_with(".csproj")
        || name.ends_with(".fsproj")
}

#[cfg(test)]
mod tests {
    use super::super::MAX_FILE_BYTES;
    use super::*;

    fn file(text: &str) -> File {
        File {
            bytes: text.as_bytes().to_vec(),
            executable: false,
        }
    }
    #[test]
    fn overlays_preserve_production_and_modes_apply_deletions_and_reject_limits() {
        let base = BTreeMap::from([
            ("src.txt".into(), file("old")),
            ("tests/old".into(), file("same")),
            ("support/removed".into(), file("fixture")),
        ]);
        let mut files = BTreeMap::from([
            ("src.txt".into(), file("new")),
            ("tests/new".into(), file("same")),
            ("tests/changed".into(), file("assertion")),
        ]);
        files.get_mut("tests/changed").unwrap().executable = true;
        let input = Snapshot {
            scope_evidence: Default::default(),
            root: "/repo".into(),
            identity: super::super::Identity {
                verification_digest: None,
                mode: "worktree".into(),
                base: "base".into(),
                head: "head".into(),
                content_digest: super::super::content_digest(&files),
                merge_request: None,
            },
            changes: super::super::compare_files(&base, &files),
            base_files: base,
            files,
            path_filter: None,
            commits: vec![],
        };
        let prepare = |input: &Snapshot, max| {
            super::prepare(
                input,
                &["src.txt".into()],
                &["tests/**".into()],
                &["support/**".into()],
                &[],
                max,
                MAX_FILE_BYTES,
            )
        };
        let ready = prepare(&input, 1024).unwrap();
        assert_eq!(ready.tests, BTreeSet::from(["tests/changed".into()]));
        assert_eq!(ready.baseline.files["src.txt"].bytes, b"old");
        assert!(ready.baseline.files["tests/changed"].executable);
        assert!(!ready.baseline.files.contains_key("tests/old"));
        assert!(!ready.baseline.files.contains_key("support/removed"));
        assert_eq!(ready.overlay["support/removed"], None);
        assert_eq!(
            ready.baseline.identity.content_digest,
            super::super::content_digest(&ready.baseline.files)
        );
        assert!(prepare(&input, 1).is_err());
        let mut oversized = input.clone();
        oversized.files.insert(
            "tests/huge".into(),
            File {
                bytes: vec![b'a'; MAX_FILE_BYTES + 1],
                executable: false,
            },
        );
        assert!(prepare(&oversized, usize::MAX).is_err());
        for limit in [0, crate::domain::snapshot_budget::MAX_FILE_BYTES + 1] {
            assert!(super::prepare(&input, &[], &[], &[], &[], usize::MAX, limit).is_err());
        }
        let mut many = input.clone();
        for i in 0..MAX_FILES {
            many.base_files.insert(format!("keep/{i}"), file(""));
        }
        assert!(prepare(&many, usize::MAX).is_err());
        for name in [
            "Cargo.toml",
            "nested/build.gradle",
            "app.csproj",
            "MODULE.bazel.lock",
        ] {
            assert!(build_input(name));
        }
        assert!(!build_input("tests/assertions.rs"));
    }
}
