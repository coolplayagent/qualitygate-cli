use super::*;

#[test]
fn repository_roots_keep_native_resolution_and_errors() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    assert_eq!(
        repository_root(&root.join(".")).unwrap(),
        root.canonicalize().unwrap()
    );
    let error = repository_root(&root.join("missing")).unwrap_err();
    assert_eq!(error.to_string(), "Repository root does not exist");
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
}

#[cfg(unix)]
#[test]
fn repository_roots_resolve_symlinks_before_input_confinement() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir(root.join("repository")).unwrap();
    std::os::unix::fs::symlink("repository", root.join("alias")).unwrap();
    assert_eq!(
        repository_root(&root.join("alias")).unwrap(),
        root.join("repository").canonicalize().unwrap()
    );
}

#[test]
fn paths_are_normalized_without_traversal_or_git_metadata() {
    assert_eq!(relative(Path::new("./src/lib.rs")).unwrap(), "src/lib.rs");
    for name in [
        "../outside",
        "/tmp/absolute",
        ".git/config",
        ".GIT/config",
        "src/../../x",
        "x\\y",
    ] {
        assert!(relative(Path::new(name)).is_err(), "{name}");
    }
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        confined(root.path(), Path::new("new/file")).unwrap(),
        root.path().join("new/file")
    );
}

#[test]
fn native_filesystem_paths_are_normalized_before_policy_comparison() {
    assert_eq!(
        from_native(&Path::new("src").join("lib.rs")).unwrap(),
        "src/lib.rs"
    );
    assert!(from_native(&Path::new("src").join("..").join("escape")).is_err());
}

#[cfg(windows)]
#[test]
fn displays_ordinary_windows_paths_without_verbatim_prefix() {
    assert_eq!(
        display(Path::new(r"\\?\C:\workspace\project\qualitygate.yaml")).to_string(),
        r"C:\workspace\project\qualitygate.yaml"
    );
    let reserved = Path::new(r"\\?\C:\CON\qualitygate.yaml");
    assert_eq!(
        display(reserved).to_string(),
        reserved.display().to_string()
    );
    let network = Path::new(r"\\?\UNC\server\share\qualitygate.yaml");
    assert_eq!(display(network).to_string(), network.display().to_string());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_ancestors_even_when_leaf_does_not_exist() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/tmp", root.path().join("link")).unwrap();
    assert!(confined(root.path(), Path::new("link/missing")).is_err());
}
