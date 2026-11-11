use super::super::test_support;
use super::*;

fn loaded(obligations: Vec<Obligation>) -> policy::Loaded {
    let bindings = obligations
        .iter()
        .map(|o| (o.id.clone(), "binding".into()))
        .collect();
    policy::Loaded {
        catalog: config::catalog::Catalog {
            entries: Default::default(),
        },
        evidence: test_support::policy(),
        plan: config::Plan {
            rules: Default::default(),
            commands: vec![],
            order: vec!["unit".into()],
            required: vec![],
            pending_delivery: vec![],
            task_id: Some("task".into()),
            acceptance: Default::default(),
            acceptance_descriptions: Default::default(),
            constraints: vec![],
            agent_obligations: obligations,
        },
        protected_paths: vec![],
        constraint_bindings: Default::default(),
        agent_bindings: bindings,
    }
}

fn obligation(snapshot: &mut snapshot::Snapshot) -> Obligation {
    Obligation {
        schema_version: 1,
        id: "verify".into(),
        version: 1,
        source: test_support::source(snapshot),
        required: true,
        scope: Scope::default(),
        triggers: vec![Event::BeforeDelivery],
        must: vec!["Run meaningful tests".into()],
        must_not: vec![],
        enforcement: Enforcement::Instruction,
        adoption: Some(test_support::adoption("binding")),
    }
}

#[test]
fn projection_includes_only_adopted_applicable_instructions_and_binds_prompt_bytes() {
    let mut snapshot = test_support::snapshot(std::path::Path::new("/unit"));
    let active = obligation(&mut snapshot);
    let mut unrelated = active.clone();
    unrelated.id = "reviewer-only".into();
    unrelated.scope.roles = vec!["reviewer".into()];
    let result = build(
        &snapshot,
        loaded(vec![active, unrelated]),
        "implementer".into(),
        vec![],
    )
    .unwrap();
    assert!(result.ready);
    assert_eq!(result.obligations.len(), 1);
    assert_eq!(result.obligations[0].status, Status::Ready);
    assert!(result.prompt.contains("Run meaningful tests"));
    assert!(!result.prompt.contains("\"id\": \"reviewer-only\""));
    assert_eq!(
        result.prompt_digest,
        snapshot::digest(result.prompt.as_bytes())
    );
    assert!(result.known_limits.iter().any(|s| s.contains("not proof")));
    let mut changed = snapshot.clone();
    changed.identity.content_digest = snapshot::digest(b"changed source");
    let result2 = build(
        &changed,
        loaded(
            result
                .obligations
                .into_iter()
                .map(|b| b.obligation)
                .collect(),
        ),
        "implementer".into(),
        vec![],
    )
    .unwrap();
    assert_eq!(result.contract_digest, result2.contract_digest);
    assert_ne!(result.prompt_digest, result2.prompt_digest);
}

#[test]
fn missing_adoption_changed_source_and_unavailable_checks_block_required_obligations() {
    let mut snapshot = test_support::snapshot(std::path::Path::new("/unit"));
    let base = obligation(&mut snapshot);
    for (case, expected) in [
        ("unadopted", Status::Unadopted),
        ("stale", Status::StaleAdoption),
        ("source", Status::SourceInvalid),
        ("check", Status::UnavailableCheck),
    ] {
        let mut obligation = base.clone();
        match case {
            "unadopted" => obligation.adoption = None,
            "stale" => obligation.adoption.as_mut().unwrap().binding_digest = "old".into(),
            "source" => obligation.source.content_hash = "old".into(),
            "check" => {
                obligation.enforcement = Enforcement::Checks {
                    checks: vec!["missing".into()],
                }
            }
            _ => unreachable!(),
        }
        for required in [true, false] {
            obligation.required = required;
            let result = build(
                &snapshot,
                loaded(vec![obligation.clone()]),
                "implementer".into(),
                vec![],
            )
            .unwrap();
            assert_eq!(result.ready, !required, "{case}");
            assert_eq!(result.obligations[0].status, expected);
            assert!(!result.prompt.contains("Run meaningful tests"));
        }
    }
}

#[test]
fn conflicting_adopted_instructions_and_prior_validation_gaps_prevent_readiness() {
    let mut snapshot = test_support::snapshot(std::path::Path::new("/unit"));
    let first = obligation(&mut snapshot);
    let mut second = first.clone();
    second.id = "conflict".into();
    second.must.clear();
    second.must_not = first.must.clone();
    let result = build(
        &snapshot,
        loaded(vec![first.clone(), second]),
        "implementer".into(),
        vec!["policy changed".into()],
    )
    .unwrap();
    assert!(!result.ready);
    assert!(result.gaps.iter().any(|gap| gap == "policy changed"));
    assert!(
        result
            .gaps
            .iter()
            .any(|gap| gap.contains("Conflicting must/must_not"))
    );
    let mut huge = first;
    huge.must = vec!["x".repeat(512 * 1024)];
    assert!(
        build(&snapshot, loaded(vec![huge]), "implementer".into(), vec![])
            .unwrap_err()
            .to_string()
            .contains("512 KiB")
    );
}

#[tokio::test]
async fn public_projection_binds_selected_repository_and_rejects_partial_scope() {
    let root = test_support::repository("schema_version: 1\n").await;
    let options = test_support::options(root.path());
    let first = project(options.clone(), "implementer".into())
        .await
        .unwrap();
    assert!(first.ready);
    assert!(first.obligations.is_empty());
    assert_eq!(
        std::path::Path::new(&first.repository)
            .canonicalize()
            .unwrap(),
        root.path().canonicalize().unwrap()
    );
    assert!(
        first
            .prompt
            .contains(&format!("Repository: {}\n", first.repository))
    );
    assert!(
        project(options.clone(), "invalid role!".into())
            .await
            .is_err()
    );
    let mut partial = options.clone();
    partial.profile = "quick".into();
    assert!(
        project(partial, "implementer".into())
            .await
            .unwrap_err()
            .to_string()
            .contains("full, unfiltered")
    );
    std::fs::write(root.path().join("source.rs"), "// subsequent edit\n").unwrap();
    let second = project(options, "implementer".into()).await.unwrap();
    assert!(second.ready);
    assert_eq!(first.contract_digest, second.contract_digest);
    assert_ne!(first.prompt_digest, second.prompt_digest);
}
