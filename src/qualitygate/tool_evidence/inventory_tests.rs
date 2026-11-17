use super::*;

#[test]
fn independently_maps_rust_modules_functions_and_simple_methods() {
    let sources = BTreeMap::from([
        ("src/lib.rs".into(), b"pub fn root() {}\nmod nested { pub fn inner() {} }\nstruct Thing; impl Thing { fn method() {} }\nmod tests { fn ignored() {} }\nfn parent() { fn local() {} }\n".to_vec()),
        ("src/sub/mod.rs".into(),b"fn sub() {}".to_vec()),
        ("src/bin/probe.rs".into(),b"fn main() {}".to_vec()),
    ]);
    let inventory = rust_inventory(&sources, "", "demo").unwrap();
    let names: BTreeSet<_> = inventory
        .entities
        .iter()
        .map(|v| v.location.symbol.as_str())
        .collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "demo::root",
            "demo::nested::inner",
            "demo::Thing::method",
            "demo::parent",
            "demo::sub::sub",
            "probe::main"
        ])
    );
    assert_eq!(inventory.files.len(), 3);
    assert_eq!(
        rust_inventory(
            &BTreeMap::from([("app/src/main.rs".into(), b"fn main() {}".to_vec())]),
            "app",
            "program"
        )
        .unwrap()
        .entities[0]
            .namespace,
        "program"
    );
}

#[test]
fn rejects_unparseable_ambiguous_and_unsupported_inventory() {
    for source in [
        b"fn missing(".as_slice(),
        b"fn same() {} fn same() {}",
        b"struct T<A>(A); impl<A> T<A> { fn a() {} }",
        &[255],
    ] {
        assert!(
            rust_inventory(
                &BTreeMap::from([("src/lib.rs".into(), source.to_vec())]),
                "",
                "demo"
            )
            .is_err()
        );
    }
    assert!(
        rust_inventory(
            &BTreeMap::from([("src/a.py".into(), b"pass".to_vec())]),
            "",
            "demo"
        )
        .is_err()
    );
    assert!(rust_inventory(&BTreeMap::from([("../a.rs".into(), vec![])]), "", "demo").is_err());
    assert!(
        rust_inventory(
            &BTreeMap::from([("src/a.rs".into(), vec![])]),
            "app",
            "demo"
        )
        .is_err()
    );
    assert!(rust_inventory(&BTreeMap::new(), "", "not a crate").is_err());
}
