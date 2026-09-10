use anyhow::{Context, Result, bail};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const EXCLUDED: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".venv",
    "venv",
    "dist",
    ".qualitygate",
    ".coverage",
    "__pycache__",
];
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL: usize = 12 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

#[derive(Default)]
pub(super) struct Inventory {
    pub files: BTreeSet<String>,
    pub manifests: BTreeMap<String, String>,
    pub digests: BTreeMap<String, String>,
    pub entries: usize,
    pub bytes_read: usize,
}

fn read(root: &Path, path: &str, total: &mut usize) -> Result<String> {
    let path = crate::paths::confined(root, Path::new(path))?;
    let file = std::fs::File::open(&path)?;
    if !file.metadata()?.is_file() {
        bail!("Discovery input must be a regular file: {}", path.display());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    *total += bytes.len();
    if bytes.len() > MAX_BYTES || *total > MAX_TOTAL {
        bail!("Discovery exceeds 2 MiB per input or 12 MiB total input budget");
    }
    String::from_utf8(bytes)
        .with_context(|| format!("Discovery input is not UTF-8: {}", path.display()))
}

pub(super) fn manifest_kind(path: &str) -> Option<(&'static str, &'static str)> {
    let file = Path::new(path).file_name()?.to_str()?;
    match file {
        "Cargo.toml" => Some(("cargo", "rust")),
        "pom.xml" => Some(("maven", "java")),
        "pyproject.toml" | "requirements.txt" | "setup.py" | "setup.cfg" => {
            Some(("python", "python"))
        }
        "package.json" => Some(("node", "typescript")),
        "go.mod" => Some(("go", "go")),
        "build.gradle" | "build.gradle.kts" => Some(("gradle", "java")),
        "Gemfile" => Some(("ruby", "ruby")),
        "composer.json" => Some(("composer", "php")),
        _ if file.ends_with(".csproj") => Some(("dotnet", "csharp")),
        _ => None,
    }
}

fn ignored(matchers: &[Arc<Gitignore>], path: &Path, directory: bool) -> bool {
    for matcher in matchers.iter().rev() {
        let matched = matcher.matched(path, directory);
        if !matched.is_none() {
            return matched.is_ignore();
        }
    }
    false
}

pub(super) fn scan(root: &Path) -> Result<Inventory> {
    let started = Instant::now();
    let mut result = Inventory::default();
    let mut pending = vec![(PathBuf::new(), Vec::<Arc<Gitignore>>::new())];
    while let Some((relative, mut matchers)) = pending.pop() {
        if started.elapsed() > Duration::from_secs(30) {
            bail!("Repository discovery exceeded its 30-second budget");
        }
        if relative.components().count() > 64 {
            bail!("Repository discovery exceeds 64 directory levels");
        }
        let directory = crate::paths::confined(root, &relative)?;
        let ignore_path = relative.join(".gitignore");
        if std::fs::symlink_metadata(root.join(&ignore_path)).is_ok() {
            let name = crate::paths::from_native(&ignore_path)?;
            let text = read(root, &name, &mut result.bytes_read)?;
            let mut builder = GitignoreBuilder::new(&directory);
            for line in text.lines() {
                builder
                    .add_line(Some(ignore_path.clone()), line)
                    .with_context(|| format!("Invalid ignore pattern in {name}"))?;
            }
            result.digests.insert(name, super::digest(text.as_bytes()));
            matchers.push(Arc::new(builder.build()?));
        }
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            result.entries += 1;
            if result.entries > MAX_ENTRIES || started.elapsed() > Duration::from_secs(30) {
                bail!("Repository discovery exceeds 20,000 entries or 30-second budget");
            }
            let kind = entry.file_type()?;
            if entry.file_name() == ".git"
                || kind.is_dir() && EXCLUDED.contains(&entry.file_name().to_string_lossy().as_ref())
            {
                continue;
            }
            if ignored(&matchers, &entry.path(), kind.is_dir()) {
                continue;
            }
            let name = crate::paths::from_native(entry.path().strip_prefix(root)?)?;
            if kind.is_symlink() || !(kind.is_file() || kind.is_dir()) {
                bail!("Discovery requires regular files and directories: {name}");
            }
            if kind.is_dir() {
                pending.push((PathBuf::from(name), matchers.clone()));
            } else {
                result.files.insert(name.clone());
                if manifest_kind(&name).is_some() {
                    let text = read(root, &name, &mut result.bytes_read)?;
                    result
                        .digests
                        .insert(name.clone(), super::digest(text.as_bytes()));
                    result.manifests.insert(name, text);
                    if result.manifests.len() > 512 {
                        bail!("Discovery exceeds 512 project manifests")
                    }
                }
            }
        }
    }
    Ok(result)
}
