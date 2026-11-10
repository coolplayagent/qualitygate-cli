use super::*;
use crate::snapshot::{File, Identity};

fn snapshot(root: &Path) -> Snapshot {
    let files = [
        (
            "src/lib.rs".into(),
            File {
                bytes: b"checked code\n".to_vec(),
                executable: false,
            },
        ),
        (
            "tools/check".into(),
            File {
                bytes: b"checked executable\n".to_vec(),
                executable: true,
            },
        ),
    ]
    .into();
    Snapshot {
        root: root.into(),
        identity: Identity {
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: crate::snapshot::content_digest(&files),
            verification_digest: None,
            merge_request: None,
        },
        files,
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: vec![],
        scope_evidence: Default::default(),
    }
}

#[test]
fn export_uses_checked_memory_preserves_modes_and_reuses_identical_objects() {
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let snapshot = snapshot(root.path());
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/lib.rs"), b"later unverified edit").unwrap();
    let first = export(&snapshot, output.path()).unwrap();
    let second = export(&snapshot, output.path()).unwrap();
    assert_eq!(first.artifact.path, second.artifact.path);
    assert_eq!(first.artifact.digest, second.artifact.digest);
    assert_eq!(first.files, 2);
    assert_eq!(
        first.snapshot.content_digest,
        snapshot.identity.content_digest
    );
    let bytes = std::fs::read(&first.artifact.path).unwrap();
    assert_eq!(first.artifact.digest, crate::snapshot::digest(&bytes));
    assert_eq!(first.artifact.bytes, bytes.len() as u64);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    for (name, mode) in [("src/lib.rs", 0o644), ("tools/check", 0o755)] {
        let mut file = archive.by_name(name).unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();
        assert_eq!(content, snapshot.files[name].bytes);
        assert_eq!(file.unix_mode().unwrap() & 0o777, mode);
    }
    assert!(
        std::fs::metadata(&first.artifact.path)
            .unwrap()
            .permissions()
            .readonly()
    );
    std::fs::remove_file(&first.artifact.path).unwrap();
    std::fs::write(&first.artifact.path, b"substituted archive").unwrap();
    assert!(
        export(&snapshot, output.path())
            .unwrap_err()
            .to_string()
            .contains("altered")
    );
}

#[test]
fn export_rejects_stale_identity_unsafe_paths_and_destination_inside_checked_inputs() {
    let root = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let mut snapshot = snapshot(root.path());
    assert!(export(&snapshot, root.path()).is_err());
    snapshot
        .files
        .get_mut("src/lib.rs")
        .unwrap()
        .bytes
        .push(b'!');
    assert!(
        export(&snapshot, output.path())
            .unwrap_err()
            .to_string()
            .contains("frozen snapshot")
    );
    snapshot.files.insert(
        "../escape".into(),
        File {
            bytes: b"unsafe".to_vec(),
            executable: false,
        },
    );
    snapshot.identity.content_digest = crate::snapshot::content_digest(&snapshot.files);
    assert!(export(&snapshot, output.path()).is_err());
    assert_eq!(output.path().read_dir().unwrap().count(), 0);
    assert!(identify(root.path()).is_err());
    let huge = output.path().join("huge");
    std::fs::File::create(&huge)
        .unwrap()
        .set_len(MAX_ARCHIVE_BYTES + 1)
        .unwrap();
    assert!(identify(&huge).is_err());
    #[cfg(unix)]
    {
        let link = output.path().join("link");
        std::os::unix::fs::symlink(&huge, &link).unwrap();
        assert!(identify(&link).is_err());
    }
}

#[test]
fn streaming_writer_refuses_exhausted_byte_and_time_budgets_before_writing() {
    let file = tempfile::tempfile().unwrap();
    let mut writer = HashingWriter {
        file: &file,
        hash: Sha256::new(),
        bytes: MAX_ARCHIVE_BYTES,
        started: Instant::now(),
    };
    assert!(writer.write_all(b"x").is_err());
    assert_eq!(file.metadata().unwrap().len(), 0);
    writer.bytes = 0;
    writer.started = Instant::now() - Duration::from_secs(121);
    assert!(writer.write_all(b"x").is_err());
    assert_eq!(file.metadata().unwrap().len(), 0);
    writer.started = Instant::now();
    writer.write_all(b"accepted").unwrap();
    writer.flush().unwrap();
    assert_eq!(writer.bytes, 8);
    assert_eq!(
        format!("sha256:{:x}", writer.hash.finalize()),
        crate::snapshot::digest(b"accepted")
    );
}
