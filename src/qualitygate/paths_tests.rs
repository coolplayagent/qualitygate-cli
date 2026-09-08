use super::*;

#[test]
fn paths_are_normalized_without_traversal_or_git_metadata() {
    assert_eq!(relative(Path::new("./src/lib.rs")).unwrap(), "src/lib.rs");
    for name in [
        "../outside",
        "/tmp/absolute",
        ".git/config",
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

#[cfg(unix)]
#[test]
fn rejects_symlink_ancestors_even_when_leaf_does_not_exist() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/tmp", root.path().join("link")).unwrap();
    assert!(confined(root.path(), Path::new("link/missing")).is_err());
}
