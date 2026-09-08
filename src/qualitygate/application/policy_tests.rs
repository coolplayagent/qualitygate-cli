use super::*;

fn source(document: &str, section: &str, expected: &str) -> (Source, BTreeMap<String, File>) {
    (
        Source {
            document: "AGENTS.md".into(),
            section: section.into(),
            content_hash: snapshot::digest(expected.as_bytes()),
        },
        [(
            "AGENTS.md".into(),
            File {
                bytes: document.as_bytes().to_vec(),
                executable: false,
            },
        )]
        .into(),
    )
}

#[test]
fn normative_hash_includes_subsections_and_fences_but_excludes_other_sections() {
    let expected = "## Policy\nUse real tests.\n### Examples\n```md\n## This is code\n```\n";
    let document = format!("# Team\nIntroduction\n{expected}## Unrelated\nNotes\n");
    let (rule, files) = source(&document, "Policy", expected);
    validate_source(&rule, &files).unwrap();
    let (_, changed) = source(
        &document.replace("Use real tests.", "Remove tests."),
        "Policy",
        expected,
    );
    assert!(validate_source(&rule, &changed).is_err());
    let (_, changed) = source(&document.replace("Notes", "New notes"), "Policy", expected);
    validate_source(&rule, &changed).unwrap();
}

#[test]
fn missing_ambiguous_and_code_only_headings_cannot_satisfy_a_source_mapping() {
    for document in [
        "plain text",
        "```md\n# Policy\n```\n",
        "# Policy\nFirst\n# Policy\nSecond\n",
        "~~~md\n# Policy\n~~~\n",
    ] {
        let (rule, files) = source(document, "Policy", document);
        assert!(validate_source(&rule, &files).is_err(), "{document}");
    }
    let (rule, _) = source("# Policy\n", "Policy", "# Policy\n");
    assert!(validate_source(&rule, &BTreeMap::new()).is_err());
}
