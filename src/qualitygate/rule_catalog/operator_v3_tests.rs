use super::*;
use serde_json::{Value, json};
fn validate(value: Value) -> Result<Validated, String> {
    serde_json::from_value::<Expr>(value)
        .map_err(|e| e.to_string())?
        .validate(&BTreeSet::from(["tests".into()]))
}
fn quantify(body: Value) -> Value {
    json!({"op":"all","input":{"op":"entities"},"as":"source","predicate":body})
}
#[test]
fn lexical_types_check_outer_references_before_execution_even_on_empty_collections() {
    let outer = json!({"op":"field","of":{"op":"var","name":"source"},"name":"path"});
    let inner = json!({"op":"field","of":{"op":"var","name":"test"},"name":"path"});
    let nested = json!({"op":"any","input":{"op":"source","name":"tests"},"as":"test","predicate":{"op":"compare","left":outer,"relation":"eq","right":inner}});
    let checked = validate(quantify(nested.clone())).unwrap();
    assert_eq!(
        checked.path_sources,
        BTreeSet::from(["entities".into(), "tests".into()])
    );
    let mut shadow = nested;
    shadow["as"] = json!("source");
    for body in [
        shadow,
        json!({"op":"var","name":"test"}),
        json!({"op":"var","name":"unknown"}),
        json!({"op":"contains","input":{"op":"entities"},"value":{"op":"string","value":"a"}}),
        json!({"op":"all","input":{"op":"source","name":"missing"},"predicate":{"op":"bool","value":true}}),
        json!({"op":"field","name":"path","of":{"op":"string","value":"no entity"}}),
        json!({"op":"map","input":{"op":"entities"},"as":"x","body":{"op":"var","name":"x"}}),
        json!({"op":"replace","value":{"op":"string","value":"x"},"from":"","to":"y"}),
    ] {
        assert!(validate(quantify(body.clone())).is_err(), "{body}");
    }
    let number_map =
        json!({"op":"map","input":{"op":"entities"},"as":"x","body":{"op":"number","value":1}});
    assert!(validate(json!({"op":"all","input":number_map,"as":"n","predicate":{"op":"compare","left":{"op":"var","name":"n"},"relation":"gt","right":{"op":"number","value":0}}})).is_ok());
    assert!(validate(json!({"op":"all","input":number_map,"predicate":{"op":"matches","value":{"op":"field","name":"name"},"pattern":".*"}})).is_err());
    let text_map =
        json!({"op":"map","input":{"op":"entities"},"as":"x","body":{"op":"field","name":"name"}});
    assert!(validate(json!({"op":"subset","left":number_map,"right":text_map})).is_err());
}
#[test]
fn bounded_v3_contract_rejects_undeclared_sources_and_legacy_extensions() {
    let yes = Expr::Bool { value: true };
    assert!(yes.validate(&BTreeSet::from(["entities".into()])).is_err());
    assert!(
        yes.validate(&(0..17).map(|i| format!("source{i}")).collect())
            .is_err()
    );
    for name in ["", "_hidden", "1test", "测试", &"a".repeat(65)] {
        assert!(identifier(name).is_err());
    }
    let mut nested = json!({"op":"bool","value":true});
    for _ in 0..17 {
        nested = json!({"op":"not","arg":nested});
    }
    let too_many = vec![json!({"op":"bool","value":true}); 65];
    for value in [
        nested,
        json!({"op":"or","args":too_many}),
        json!({"op":"concat","args":[]}),
        json!({"op":"matches","value":{"op":"string","value":"x"},"pattern":"["}),
        json!({"op":"matches","value":{"op":"string","value":"x"},"pattern":"x".repeat(1025)}),
        json!({"op":"matches","value":{"op":"string","value":"界".repeat(1366)},"pattern":".*"}),
        json!({"op":"compare","left":{"op":"string","value":"a"},"relation":"lt","right":{"op":"string","value":"b"}}),
        json!({"op":"all","input":{"op":"entities"},"as":"x","predicate":{"op":"var","name":"x"}}),
    ] {
        assert!(validate(value.clone()).is_err(), "{value}");
    }
    let v3: Expr = serde_json::from_value(quantify(json!({"op":"bool","value":true}))).unwrap();
    assert!(v3.legacy().is_err());
    assert!(yes.legacy().unwrap().validate().is_ok());
}
