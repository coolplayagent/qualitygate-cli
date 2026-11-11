use super::*;

#[test]
fn oversized_serialized_reports_never_publish_a_new_index() {
    let root = tempfile::tempdir().unwrap();
    assert!(Store::optional(root.path()).unwrap().is_none());
    Store::transaction(root.path(), |store| store.put_blob(b"anchor")).unwrap();
    let head = std::fs::read(root.path().join(DIRECTORY).join("HEAD")).unwrap();
    assert!(
        Store::transaction(root.path(), |store| store
            .put_record("report", &"x".repeat(MAX_OBJECT_BYTES)))
        .is_err()
    );
    assert_eq!(
        std::fs::read(root.path().join(DIRECTORY).join("HEAD")).unwrap(),
        head
    );
}

#[test]
fn content_objects_are_deduplicated_and_failed_transactions_leave_head_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let reference = Store::transaction(root.path(), |store| {
        let first = store.put_blob(b"retained")?;
        assert_eq!(first, store.put_blob(b"retained")?);
        Ok(first)
    })
    .unwrap();
    let store = Store::open(root.path()).unwrap();
    assert_eq!(store.index.objects, 2); // Blob and immutable index.
    let head = std::fs::read(root.path().join(DIRECTORY).join("HEAD")).unwrap();
    let index_bytes = store.blob(std::str::from_utf8(&head).unwrap()).unwrap();
    assert_eq!(store.index.object_bytes, (index_bytes.len() + 8) as u64);
    assert_eq!(store.blob(&reference).unwrap(), b"retained");
    let failed: Result<()> = Store::transaction(root.path(), |store| {
        store.put_blob(b"unpublished")?;
        bail!("fixture failure")
    });
    assert!(failed.is_err());
    assert_eq!(
        std::fs::read(root.path().join(DIRECTORY).join("HEAD")).unwrap(),
        head
    );
    assert!(!root.path().join(DIRECTORY).join("write.lock").exists());
    assert_eq!(Store::open(root.path()).unwrap().index.sequence, 1);
    Store::transaction(root.path(), |store| {
        store.put_blob(b"unpublished")?;
        Ok(())
    })
    .unwrap();
    let recovered = Store::open(root.path()).unwrap();
    assert_eq!(recovered.index.objects, 4); // Two blobs and two index versions.
    assert_eq!(recovered.index.object_inventory.len(), 2);
}

#[test]
fn invalid_objects_types_budgets_and_expiration_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    Store::transaction(root.path(), |store| {
        let reference = store.put_record("fixture", &42)?;
        assert!(store.record::<u32>(&reference, "other").is_err());
        assert!(store.blob("../../outside").is_err());
        assert!(store.blob(&digest(b"missing")).is_err());
        assert!(store.put_blob(&vec![0; MAX_OBJECT_BYTES + 1]).is_err());
        let count = store.index.objects;
        store.index.objects = MAX_OBJECTS;
        assert!(store.put_blob(b"over object budget").is_err());
        store.index.objects = count;
        let total = store.index.object_bytes;
        store.index.object_bytes = MAX_ARCHIVE_BYTES;
        assert!(store.put_blob(b"over byte budget").is_err());
        store.index.object_bytes = total;
        std::fs::write(store.object_path(&reference)?, b"tampered")?;
        assert!(store.blob(&reference).is_err());
        assert!(store.put_record("fixture", &42).is_err());
        let path = store.object_path(&digest(b"huge"))?;
        std::fs::write(&path, vec![0; MAX_OBJECT_BYTES + 1])?;
        assert!(bounded(&path).is_err());
        assert!(bounded(path.parent().unwrap()).is_err());
        let deadline = store.deadline;
        store.deadline = Instant::now();
        assert!(store.blob(&reference).is_err());
        store.deadline = deadline;
        Ok(())
    })
    .unwrap();
}

#[test]
fn optimistic_head_conflicts_and_corrupt_indexes_are_not_overwritten() {
    let root = tempfile::tempdir().unwrap();
    Store::transaction(root.path(), |store| {
        store.put_blob(b"original")?;
        Ok(())
    })
    .unwrap();
    let head = root.path().join(DIRECTORY).join("HEAD");
    assert!(
        Store::transaction(root.path(), |store| {
            store.put_blob(b"new")?;
            std::fs::write(&head, b"another writer")?;
            Ok(())
        })
        .unwrap_err()
        .to_string()
        .contains("changed during transaction")
    );
    assert_eq!(std::fs::read(&head).unwrap(), b"another writer");
    assert!(Store::open(root.path()).is_err());
    std::fs::remove_file(&head).unwrap();
    assert!(
        Store::transaction(root.path(), |store| {
            store.put_blob(b"initial")?;
            std::fs::write(&head, b"new competing head")?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(std::fs::read(&head).unwrap(), b"new competing head");
}

#[cfg(unix)]
#[test]
fn symlink_ancestors_objects_and_heads_are_rejected() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join(".qualitygate")).unwrap();
    assert!(Store::transaction(root.path(), |_| Ok(())).is_err());
    assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
    std::fs::remove_file(root.path().join(".qualitygate")).unwrap();
    Store::transaction(root.path(), |store| {
        let reference = store.put_blob(b"test")?;
        let path = store.object_path(&reference)?;
        std::fs::remove_file(&path)?;
        symlink(outside.path(), &path)?;
        assert!(store.blob(&reference).is_err());
        Ok(())
    })
    .unwrap();
    let head = root.path().join(DIRECTORY).join("HEAD");
    std::fs::remove_file(&head).unwrap();
    symlink(outside.path(), &head).unwrap();
    assert!(Store::open(root.path()).is_err());
}
