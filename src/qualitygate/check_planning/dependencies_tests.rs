use super::*;
use crate::config::{Config, Plan};
use serde_json::json;

fn graph(nodes: &[(String, Vec<String>)]) -> Result<Graph<'_>> {
    Graph::new(
        nodes
            .iter()
            .map(|(id, dependencies)| (id.as_str(), dependencies.as_slice())),
    )
}

#[test]
fn stable_order_agrees_with_first_eligible_policy_order_for_varied_dags() {
    for seed in 0..128 {
        let nodes: Vec<_> = (0..8)
            .map(|i| {
                (
                    format!("check-{i}"),
                    (i + 1..8)
                        .filter(|j| (seed + i * 17 + j * 11) % (j + 2) < 2)
                        .map(|j| format!("check-{j}"))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let mut pending = nodes.clone();
        let mut expected = Vec::new();
        while !pending.is_empty() {
            let position = pending
                .iter()
                .position(|(_, dependencies)| dependencies.iter().all(|id| expected.contains(id)))
                .unwrap();
            expected.push(pending.remove(position).0);
        }
        assert_eq!(
            graph(&nodes).unwrap().order().unwrap(),
            expected,
            "seed {seed}"
        );
    }
}

#[test]
fn invalid_graphs_remain_errors_instead_of_partial_plans() {
    for nodes in [
        vec![("a".into(), vec![]), ("a".into(), vec![])],
        vec![("a".into(), vec!["missing".into()])],
        vec![
            ("a".into(), vec!["b".into(), "b".into()]),
            ("b".into(), vec![]),
        ],
    ] {
        assert!(graph(&nodes).is_err());
    }
    for nodes in [
        vec![("a".into(), vec!["a".into()])],
        vec![
            ("a".into(), vec!["b".into()]),
            ("b".into(), vec!["a".into()]),
        ],
    ] {
        assert!(graph(&nodes).unwrap().order().is_err());
    }
    let empty = graph(&[]).unwrap();
    assert!(empty.order().unwrap().is_empty());
    assert!(empty.closure(&BTreeSet::from(["missing".into()])).is_err());
}

#[test]
fn large_reverse_chain_is_iterative_and_reaches_every_prerequisite_once() {
    let count = 10_000;
    let nodes: Vec<_> = (0..count)
        .map(|i| {
            (
                format!("check-{i}"),
                if i + 1 < count {
                    vec![format!("check-{}", i + 1)]
                } else {
                    vec![]
                },
            )
        })
        .collect();
    let graph = graph(&nodes).unwrap();
    let order = graph.order().unwrap();
    assert_eq!(order.len(), count);
    assert_eq!(order[0], "check-9999");
    assert_eq!(order[count - 1], "check-0");
    let closure = graph
        .closure(&BTreeSet::from(["check-0".into(), "check-5000".into()]))
        .unwrap();
    assert_eq!(closure.len(), count);
    assert!(nodes.iter().all(|(id, _)| closure.contains(id)));
}

#[test]
fn planning_preserves_profile_closure_pending_checks_and_disabled_errors() {
    let mut config: Config = serde_json::from_value(json!({"schema_version":1,"checks":[
        {"id":"consumer","argv":["git"],"depends_on":["producer"]},
        {"id":"unrelated","argv":["git"]},
        {"id":"producer","argv":["git"]}
    ],"profiles":{"quick":{"include":["consumer"]}}}))
    .unwrap();
    let plan = Plan::build(&config, None, "quick").unwrap();
    assert_eq!(plan.order, ["producer", "consumer"]);
    assert_eq!(plan.pending_delivery, ["unrelated"]);
    assert_eq!(
        Plan::build(&config, None, "full").unwrap().order,
        ["unrelated", "producer", "consumer"]
    );
    config.rules.insert(
        "line-ending".into(),
        serde_json::from_value(json!({"enabled":false})).unwrap(),
    );
    config.checks[2].depends_on.push("line-ending".into());
    assert!(
        Plan::build(&config, None, "full")
            .unwrap_err()
            .to_string()
            .contains("disabled or unavailable")
    );
}
