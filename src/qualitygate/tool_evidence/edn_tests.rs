use super::*;

#[test]
fn accepts_audited_values_without_float_conversion() {
    let mut map = parse(br#"{:rows [{:name "a\"b", :score 0.800000000000000001}], :missing nil}"#)
        .unwrap()
        .map()
        .unwrap();
    assert!(matches!(take(&mut map, "missing").unwrap(), Value::Nil));
    let row = take(&mut map, "rows")
        .unwrap()
        .list()
        .unwrap()
        .pop()
        .unwrap();
    let mut row = row.map().unwrap();
    assert_eq!(take(&mut row, "name").unwrap().string().unwrap(), "a\"b");
    assert_eq!(
        take(&mut row, "score").unwrap().number().unwrap(),
        "0.800000000000000001"
    );
    assert!(take(&mut row, "absent").is_err());
    assert!(Value::Nil.map().is_err());
    assert!(Value::Nil.list().is_err());
    assert!(Value::Nil.string().is_err());
    assert!(Value::Nil.number().is_err());
}

#[test]
fn rejects_executable_unknown_duplicate_and_truncated_edn() {
    for text in [
        "",
        "[",
        "{",
        "nil extra",
        "{:a 1 :a 2}",
        "{\"a\" 1}",
        "{:$bad 1}",
        "{: 1}",
        "[true]",
        "[nope]",
        "[#inst \"x\"]",
        "#=(run)",
        "\"unterminated",
        "\"bad\\x\"",
        "[NaN]",
        "[1.1e999]",
    ] {
        assert!(parse(text.as_bytes()).is_err(), "accepted {text}");
    }
    assert!(parse(&[255]).is_err());
    assert!(parse(format!("{}nil{}", "[".repeat(18), "]".repeat(18)).as_bytes()).is_err());
    assert!(parse(format!("[{}]", "nil ".repeat(50_001)).as_bytes()).is_err());
    assert!(parse(format!("\"{}\"", "x".repeat(16 * 1024)).as_bytes()).is_err());
    assert!(parse(format!("[{}]", "1".repeat(4097)).as_bytes()).is_err());
    assert!(parse(&vec![b' '; 16 * 1024 * 1024 + 1]).is_err());
}
