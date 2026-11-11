//! Resolves command roots outside the asynchronous executor's worker threads.

use anyhow::Result;
use std::path::PathBuf;

pub async fn resolve(root: PathBuf) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || crate::paths::repository_root(&root)).await?
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn resolves_native_roots_and_preserves_missing_root_context() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        assert_eq!(
            super::resolve(root.join(".")).await.unwrap(),
            root.canonicalize().unwrap()
        );
        let error = super::resolve(root.join("missing")).await.unwrap_err();
        assert_eq!(error.to_string(), "Repository root does not exist");
        assert!(error.downcast_ref::<std::io::Error>().is_some());
    }
}
