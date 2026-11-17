use super::*;

#[test]
fn exact_order_handles_scale_sign_and_full_precision_without_overflow() {
    let parse = |v| Decimal::parse(v).unwrap();
    assert_eq!(parse("1.000"), parse("1e0"));
    assert_eq!(parse("10e-19"), parse("0.000000000000000001"));
    assert_eq!(parse("1e2"), Decimal::from_i64(100));
    assert_eq!(parse("0.000"), Decimal::from_i64(0));
    for (left, right) in [
        ("0", "0.000000000000000001"),
        ("-2", "-1.9"),
        ("-1", "0"),
        ("0.799999999999999999", "0.8"),
        (
            "99999999999999999999999999999999999998",
            "99999999999999999999999999999999999999",
        ),
    ] {
        assert!(parse(left) < parse(right));
        assert!(parse(right) > parse(left));
    }
    assert_eq!(parse("-9223372036854775808").as_i64().unwrap(), i64::MIN);
    assert!(parse("9223372036854775808").as_i64().is_err());
    assert!(parse("0.1").as_i64().is_err());
}

#[test]
fn malformed_overprecise_and_noncanonical_numbers_are_rejected() {
    for token in [
        "",
        "-",
        "+1",
        "NaN",
        "Infinity",
        "01",
        "-0",
        "-0.00",
        ".1",
        "1.",
        "1.2.3",
        "1e",
        "1e129",
        "1e-129",
        "1e38",
        "1e-19",
        "100000000000000000000000000000000000000",
    ] {
        assert!(Decimal::parse(token).is_err(), "accepted {token}");
    }
    assert!(Decimal::parse(&"1".repeat(129)).is_err());
    for value in [
        Decimal {
            coefficient: "-0".into(),
            scale: 0,
        },
        Decimal {
            coefficient: "0".into(),
            scale: 1,
        },
        Decimal {
            coefficient: "00".into(),
            scale: 0,
        },
        Decimal {
            coefficient: "10".into(),
            scale: 1,
        },
        Decimal {
            coefficient: "1".into(),
            scale: 19,
        },
        Decimal {
            coefficient: "x".into(),
            scale: 0,
        },
    ] {
        assert!(value.validate().is_err());
    }
    let value = Decimal::parse("123.45").unwrap();
    assert_eq!(
        serde_json::from_slice::<Decimal>(&serde_json::to_vec(&value).unwrap()).unwrap(),
        value
    );
    assert!(serde_json::from_str::<Decimal>(r#"{"coefficient":"1","scale":0,"extra":1}"#).is_err());
}
