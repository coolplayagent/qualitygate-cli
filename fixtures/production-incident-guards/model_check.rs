// Standalone, finite models for an isolated repository. See the CodeSpec experiment.
use std::{collections::BTreeSet, io};

mod subject;

struct State {
    active: u8,
    retained: BTreeSet<u8>,
}

#[derive(Clone, Copy)]
struct Plan {
    generation: u8,
    eligible_at_prepare: bool,
}

type Cases = Vec<(String, bool)>;

fn configuration_cases() -> Cases {
    let limit = 2;
    let last_good = [9];
    [
        ("below-limit", vec![4], vec![4]),
        ("exact-limit", vec![4, 5], vec![4, 5]),
        ("duplicate", vec![4, 4], last_good.to_vec()),
        ("over-limit", vec![4, 5, 6], last_good.to_vec()),
    ]
    .into_iter()
    .map(|(name, candidate, expected)| {
        let mut active = last_good.to_vec();
        subject::publish_features(&mut active, &candidate, limit);
        (name.into(), active == expected)
    })
    .collect()
}

// O/o: prepare/apply old plan; N/n: prepare/apply new plan; C: collect older payloads.
fn validate_schedule(schedule: &[u8]) -> bool {
    let mut state = State {
        active: 0,
        retained: BTreeSet::from([0, 1, 2]),
    };
    let mut prepared = [None; 2];
    for &event in schedule {
        let previous = state.active;
        match event {
            b'O' | b'N' => {
                let slot = usize::from(event == b'N');
                let generation = [1, 2][slot];
                prepared[slot] = Some(Plan {
                    generation,
                    eligible_at_prepare: generation > state.active,
                });
            }
            b'o' | b'n' => {
                let slot = usize::from(event == b'n');
                subject::apply_plan(&mut state, prepared[slot].expect("prepare precedes apply"));
            }
            b'C' => state
                .retained
                .retain(|generation| *generation >= state.active),
            _ => unreachable!("fixed event alphabet"),
        }
        if state.active < previous || !state.retained.contains(&state.active) {
            return false;
        }
    }
    state.active == 2
}

fn append_schedules(prefix: &mut Vec<u8>, cases: &mut Cases) {
    if prefix.len() == 5 {
        let name = String::from_utf8(prefix.clone()).expect("ASCII event alphabet");
        cases.push((name, validate_schedule(prefix)));
        return;
    }
    for &event in b"OoNnC" {
        if prefix.contains(&event)
            || (event == b'o' && !prefix.contains(&b'O'))
            || (event == b'n' && !prefix.contains(&b'N'))
        {
            continue;
        }
        prefix.push(event);
        append_schedules(prefix, cases);
        prefix.pop();
    }
}

fn main() -> io::Result<()> {
    let (suite, cases) = match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            println!("incident-model 1");
            return Ok(());
        }
        Some("configuration") => ("configuration", configuration_cases()),
        Some("schedules") => {
            let mut cases = Vec::new();
            append_schedules(&mut Vec::new(), &mut cases);
            ("schedules", cases)
        }
        _ => {
            return Err(io::Error::other(
                "expected configuration, schedules or --version",
            ));
        }
    };
    let failures = cases.iter().filter(|(_, passed)| !passed).count();
    let mut xml = format!(
        "<testsuite name='{suite}' tests='{}' failures='{failures}'>",
        cases.len()
    );
    for (name, passed) in cases {
        let outcome = if passed {
            ""
        } else {
            "<failure message='model invariant failed'/>"
        };
        xml.push_str(&format!("<testcase name='{name}'>{outcome}</testcase>"));
    }
    xml.push_str("</testsuite>");
    std::fs::create_dir_all("target")?;
    std::fs::write(format!("target/{suite}.xml"), xml)
}
