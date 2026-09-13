//! Confined repository paths and disposable execution directories.

use anyhow::{Context, Result, bail};
use std::path::{Component, Path, PathBuf};

/// Converts a user path into a portable, confined repository-relative name.
pub fn relative(path: &Path) -> Result<String> {
    if path
        .to_str()
        .context("Paths must be valid UTF-8")?
        .contains('\\')
    {
        bail!("Repository-relative paths must use forward slashes");
    }
    from_native(path)
}

/// Normalizes an OS-produced relative path after root confinement. Windows
/// separators are structural here; user policy paths go through relative.
pub fn from_native(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for part in path.components() {
        match part {
            Component::Normal(value) => {
                let value = value.to_str().context("Paths must be valid UTF-8")?;
                if value.eq_ignore_ascii_case(".git") || value.contains(['\\', '\0', '\n', '\r']) {
                    bail!("Unsafe repository path: {}", path.display());
                }
                parts.push(value);
            }
            Component::CurDir => {}
            _ => bail!("Path must stay inside the repository: {}", path.display()),
        }
    }
    Ok(parts.join("/"))
}

/// Rejects symlink ancestors as well as lexical traversal before reading/writing.
pub fn confined(root: &Path, path: &Path) -> Result<PathBuf> {
    let name = relative(path)?;
    let mut result = root.to_path_buf();
    for part in name.split('/').filter(|part| !part.is_empty()) {
        result.push(part);
        match std::fs::symlink_metadata(&result) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!(
                    "Symlinks are not valid checked inputs: {}",
                    result.display()
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| result.display().to_string()),
        }
    }
    Ok(result)
}

/// Allocates a unique durable evidence directory outside the checked inputs.
pub fn run_directory(base: Option<&Path>) -> Result<PathBuf> {
    let base = base
        .map(Path::to_path_buf)
        .unwrap_or_else(crate::env::artifact_base);
    std::fs::create_dir_all(&base)?;
    Ok(tempfile::Builder::new()
        .prefix("run-")
        .tempdir_in(base)?
        .keep())
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
