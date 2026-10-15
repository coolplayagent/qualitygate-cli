use super::*;
use serde_json::{Value, json};

fn expression(value: Value) -> Expr {
    serde_json::from_value(value).unwrap()
}

#[test]
fn types_and_field_binding_are_checked_before_boolean_evaluation() {
    let entities = json!({"op":"entities"});
    let field = json!({"op":"field","name":"name"});
    let predicate = json!({"op":"matches","value":field,"pattern":"^test_"});
    let valid = expression(json!({"op":"and","args":[
        {"op":"all","input":entities,"predicate":predicate},
        {"op":"compare","left":{"op":"count","input":entities},"relation":"ge","right":{"op":"number","value":1}}
    ]}));
    assert_eq!(valid.validate().unwrap().patterns.len(), 1);
    for value in [
        json!({"op":"field","name":"name"}),
        json!({"op":"and","args":[{"op":"bool","value":false},{"op":"number","value":1}]}),
        json!({"op":"all","input":{"op":"string","value":"wrong"},"predicate":{"op":"bool","value":true}}),
        json!({"op":"all","input":entities,"predicate":{"op":"number","value":1}}),
        json!({"op":"count","input":entities}),
        json!({"op":"not","arg":{"op":"string","value":"wrong"}}),
        json!({"op":"compare","left":{"op":"bool","value":true},"relation":"eq","right":{"op":"number","value":1}}),
        json!({"op":"compare","left":{"op":"string","value":"a"},"relation":"lt","right":{"op":"string","value":"b"}}),
        json!({"op":"compare","left":entities,"relation":"eq","right":entities}),
        json!({"op":"in","value":{"op":"number","value":1},"set":[{"type":"string","value":"1"}]}),
        json!({"op":"matches","value":{"op":"bool","value":true},"pattern":".*"}),
        json!({"op":"matches","value":{"op":"string","value":"s"},"pattern":"["}),
        json!({"op":"and","args":[]}),
        json!({"op":"in","value":{"op":"bool","value":true},"set":[]}),
    ] {
        assert!(expression(value.clone()).validate().is_err(), "{value}");
    }
    for value in [
        json!({"op":"exec","argv":["command"]}),
        json!({"op":"field","name":"dynamic"}),
        json!({"op":"bool","value":true,"hidden":1}),
    ] {
        assert!(serde_json::from_value::<Expr>(value).is_err());
    }
}

#[test]
fn shared_structure_and_regex_limits_reject_deep_large_and_expensive_programs() {
    let mut nested = expression(json!({"op":"bool","value":true}));
    for _ in 0..MAX_DEPTH {
        nested = Expr::Not {
            arg: Box::new(nested),
        };
    }
    assert!(nested.validate().is_ok());
    assert!(
        Expr::Not {
            arg: Box::new(nested)
        }
        .validate()
        .is_err()
    );
    let yes = Expr::Bool { value: true };
    assert!(
        Expr::And {
            args: vec![yes.clone(); 65]
        }
        .validate()
        .is_err()
    );
    assert!(
        Expr::And {
            args: vec![Expr::Or { args: vec![yes; 4] }; 64]
        }
        .validate()
        .is_err()
    );
    for text in ["x".repeat(4097), "界".repeat(1366)] {
        assert!(
            Expr::Matches {
                value: Box::new(Expr::String { value: text }),
                pattern: ".*".into()
            }
            .validate()
            .is_err()
        );
    }
    for pattern in ["x".repeat(1025), "a{100000}".into()] {
        assert!(
            Expr::Matches {
                value: Box::new(Expr::String {
                    value: String::new()
                }),
                pattern
            }
            .validate()
            .is_err()
        );
    }
    for values in [
        vec![Scalar::Bool(true); 65],
        vec![Scalar::String("x".repeat(4097))],
    ] {
        assert!(
            Expr::In {
                value: Box::new(Expr::Bool { value: true }),
                set: values
            }
            .validate()
            .is_err()
        );
    }
    let path = expression(
        json!({"op":"any","input":{"op":"entities"},"predicate":{"op":"matches","value":{"op":"field","name":"path"},"pattern":".*"}}),
    );
    assert!(path.validate().unwrap().requires_path);
}
