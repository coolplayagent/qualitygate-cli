use super::*;

fn entry(size: usize) -> Entry {
    Entry {
        path: "source.txt".into(),
        executable: false,
        oid: "a".repeat(40),
        size,
    }
}

fn size_line(size: usize) -> String {
    format!("{} blob {size}\n", "a".repeat(40))
}

#[test]
fn preflight_bounds_bytes_and_file_counts_before_acquiring_content() {
    let planned = batches(
        (0..5).map(|_| entry(0)).collect(),
        size_line(MAX_FILE_BYTES).repeat(5).as_bytes(),
        12 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(planned.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2, 1]);
    for batch in &planned {
        let wire_bytes: usize = batch
            .iter()
            .map(|entry| entry.size + entry.oid.len() + 32)
            .sum();
        assert!(wire_bytes < crate::runner::MAX_OUTPUT_BYTES);
    }
    let empty = batches(
        (0..1025).map(|_| entry(0)).collect(),
        size_line(0).repeat(1025).as_bytes(),
        1,
    )
    .unwrap();
    assert_eq!(empty.iter().map(Vec::len).collect::<Vec<_>>(), [1024, 1]);
    for (size, budget, reason) in [
        (MAX_FILE_BYTES + 1, usize::MAX, "File exceeds"),
        (2, 1, "Snapshot exceeds"),
    ] {
        assert!(
            batches(vec![entry(0)], size_line(size).as_bytes(), budget)
                .unwrap_err()
                .to_string()
                .contains(reason)
        );
    }
}

#[test]
fn malformed_missing_and_mismatched_objects_never_produce_partial_snapshots() {
    for bytes in [
        "",
        "missing\n",
        "a tree 0\n",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob nope\n",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb blob 1\n",
    ] {
        assert!(
            batches(vec![entry(0)], bytes.as_bytes(), 100).is_err(),
            "{bytes}"
        );
    }
    assert!(
        batches(
            vec![entry(0)],
            format!("{}trailing", size_line(0)).as_bytes(),
            1
        )
        .is_err()
    );
    for tail in ["", "x", "x!", "x\ntrailing"] {
        assert!(blobs(vec![entry(1)], format!("{}{tail}", size_line(1)).as_bytes()).is_err());
    }
    assert!(blobs(vec![entry(1)], format!("{}\n", size_line(0)).as_bytes()).is_err());
    let valid = blobs(vec![entry(3)], format!("{}a\0b\n", size_line(3)).as_bytes()).unwrap();
    assert_eq!(valid["source.txt"].bytes, b"a\0b");
    for listing in [
        "malformed",
        "100644\tfile\0",
        "120000 blob aaa\tfile\0",
        "100644 blob nope\tfile\0",
    ] {
        assert!(entries(listing.as_bytes(), false).is_err());
    }
    let conflict = format!("100644 {} 1\tfile\0", "a".repeat(40));
    assert!(
        entries(conflict.as_bytes(), true)
            .unwrap_err()
            .to_string()
            .contains("conflict")
    );
    let too_many = format!("100644 blob {}\tfile\0", "a".repeat(40)).repeat(MAX_FILES + 1);
    assert!(
        entries(too_many.as_bytes(), false)
            .unwrap_err()
            .to_string()
            .contains("files")
    );
}
