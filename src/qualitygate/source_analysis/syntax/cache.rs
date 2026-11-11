//! Bounded, invocation-owned syntax facts; paths and exact content bind every hit.
use super::{Structure, parse_before};
use anyhow::{Result, ensure};
use std::{
    collections::BTreeMap,
    mem::size_of,
    sync::{Arc, Mutex},
    time::Instant,
};

const MAX_ENTRIES: usize = 4096;
const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Default)]
pub struct Cache {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    entries: BTreeMap<(String, String), (Arc<Structure>, usize)>,
    bytes: usize,
}

impl Cache {
    /// A cache hit never bypasses the caller's deadline. Errors are never cached.
    pub fn parse_before(
        &self,
        path: &str,
        bytes: &[u8],
        deadline: Instant,
    ) -> Result<Option<Arc<Structure>>> {
        check_deadline(path, deadline)?;
        let key = (path.to_owned(), crate::snapshot::digest(bytes));
        check_deadline(path, deadline)?;
        if let Some((view, _)) = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Syntax cache state unavailable"))?
            .entries
            .get(&key)
        {
            let view = Arc::clone(view);
            check_deadline(path, deadline)?;
            return Ok(Some(view));
        }
        let Some(view) = parse_before(path, bytes, deadline)? else {
            return Ok(None);
        };
        let view = Arc::new(view);
        let charge = retained_bytes(&view)
            .saturating_add(key.0.capacity())
            .saturating_add(key.1.capacity())
            .saturating_add(256);
        check_deadline(path, deadline)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Syntax cache state unavailable"))?;
        retain(&mut state, key, &view, charge, MAX_ENTRIES, MAX_BYTES);
        check_deadline(path, deadline)?;
        Ok(Some(view))
    }
}

fn check_deadline(path: &str, deadline: Instant) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "Syntax parsing exceeded its budget: {path}"
    );
    Ok(())
}

fn retain(
    state: &mut State,
    key: (String, String),
    view: &Arc<Structure>,
    charge: usize,
    max_entries: usize,
    max_bytes: usize,
) {
    if charge > max_bytes || max_entries == 0 || state.entries.contains_key(&key) {
        return;
    }
    // Eviction changes reuse only. A source too large to retain is still parsed
    // with the same syntax and deadline limits, never reported clean by default.
    while state.entries.len() >= max_entries || state.bytes.saturating_add(charge) > max_bytes {
        let Some((_, (_, removed))) = state.entries.pop_first() else {
            break;
        };
        state.bytes -= removed;
    }
    state.bytes += charge;
    state.entries.insert(key, (Arc::clone(view), charge));
}

fn retained_bytes(view: &Structure) -> usize {
    let mut bytes = size_of::<Structure>() + view.language.capacity();
    bytes = bytes.saturating_add(
        view.nonblank_lines
            .capacity()
            .saturating_mul(size_of::<usize>()),
    );
    bytes = bytes.saturating_add(
        view.tests
            .capacity()
            .saturating_mul(size_of::<super::Entity>()),
    );
    bytes = bytes.saturating_add(
        view.comments
            .capacity()
            .saturating_mul(size_of::<super::Comment>()),
    );
    bytes = bytes.saturating_add(
        view.imports
            .capacity()
            .saturating_mul(size_of::<super::Import>()),
    );
    for test in &view.tests {
        for text in [
            &test.name,
            &test.symbol,
            &test.body_digest,
            &test.shape_digest,
        ] {
            bytes = bytes.saturating_add(text.capacity());
        }
        for (name, value) in &test.annotations {
            bytes = bytes
                .saturating_add(256)
                .saturating_add(name.capacity())
                .saturating_add(value.capacity());
        }
    }
    for text in view
        .comments
        .iter()
        .map(|item| &item.text)
        .chain(view.imports.iter().map(|item| &item.text))
    {
        bytes = bytes.saturating_add(text.capacity());
    }
    bytes
}

pub(crate) fn parse_cached(
    cache: Option<&Cache>,
    path: &str,
    bytes: &[u8],
    deadline: Instant,
) -> Result<Option<Arc<Structure>>> {
    match cache {
        Some(cache) => cache.parse_before(path, bytes, deadline),
        None => parse_before(path, bytes, deadline).map(|view| view.map(Arc::new)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    #[test]
    fn syntax_reuse_is_bound_to_path_content_and_invocation() {
        let cache = Cache::default();
        let source = b"#[test]\nfn verifies_contract() {}\n";
        let first = cache
            .parse_before("tests/a.rs", source, deadline())
            .unwrap()
            .unwrap();
        let again = cache
            .parse_before("tests/a.rs", source, deadline())
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        let changed = cache
            .parse_before(
                "tests/a.rs",
                b"#[test]\nfn changed_contract() {}\n",
                deadline(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(changed.tests[0].name, "changed_contract");
        assert!(!Arc::ptr_eq(&first, &changed));
        let renamed = cache
            .parse_before("tests/b.rs", source, deadline())
            .unwrap()
            .unwrap();
        assert!(!Arc::ptr_eq(&first, &renamed));
        let other_run = Cache::default()
            .parse_before("tests/a.rs", source, deadline())
            .unwrap()
            .unwrap();
        assert!(!Arc::ptr_eq(&first, &other_run));
        assert!(
            cache
                .parse_before("tests/a.rs", source, Instant::now())
                .is_err()
        );
    }

    #[test]
    fn syntax_errors_are_not_cached_as_success_and_unsupported_files_stay_absent() {
        let cache = Cache::default();
        for _ in 0..2 {
            assert!(
                cache
                    .parse_before("bad.py", b"def broken(\n", deadline())
                    .is_err()
            );
        }
        assert!(
            cache
                .parse_before("binary.rs", b"\xff", deadline())
                .is_err()
        );
        assert!(
            cache
                .parse_before("notes.txt", b"hello", deadline())
                .unwrap()
                .is_none()
        );
        assert!(cache.state.lock().unwrap().entries.is_empty());
    }

    #[test]
    fn bounded_retention_evicts_facts_without_changing_their_result() {
        let view = Arc::new(
            parse_before("a.py", b"def test_one():\n    pass\n", deadline())
                .unwrap()
                .unwrap(),
        );
        let mut state = State::default();
        let key = |path: &str| (path.to_owned(), "digest".to_owned());
        retain(&mut state, key("a"), &view, 5, 2, 12);
        retain(&mut state, key("a"), &view, 5, 2, 12);
        assert_eq!(state.bytes, 5);
        retain(&mut state, key("b"), &view, 5, 2, 12);
        retain(&mut state, key("c"), &view, 5, 2, 12);
        assert_eq!(state.entries.len(), 2);
        assert!(!state.entries.contains_key(&key("a")));
        retain(&mut state, key("large"), &view, 13, 2, 12);
        assert_eq!(state.bytes, 10);
        retain(&mut state, key("d"), &view, 12, 2, 12);
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[&key("d")].0.tests[0].name, "test_one");
        retain(&mut state, key("e"), &view, 1, 0, 12);
        assert_eq!(state.entries.len(), 1);
        assert!(retained_bytes(&view) > 0);
    }
}
