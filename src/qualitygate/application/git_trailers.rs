//! Acquire Git history once, before commands, only when a selected binding needs it.

use crate::{
    adapters::git_trailers::{self, GitFacts},
    config::{Plan, catalog::Catalog},
    snapshot::{self, Snapshot},
};
use std::sync::Arc;

pub(super) type Input = Result<Arc<GitFacts>, String>;

pub(super) async fn load(
    plan: &Plan,
    catalog: &Catalog,
    snapshot: &Arc<Snapshot>,
) -> Option<Input> {
    if !plan
        .rules
        .iter()
        .any(|(id, rule)| git_trailers::needed(rule, &catalog.entries[id]))
    {
        return None;
    }
    let history = match snapshot::history::capture(snapshot).await {
        Ok(history) => history,
        Err(error) => return Some(Err(format!("{error:#}"))),
    };
    let snapshot = snapshot.clone();
    Some(
        tokio::task::spawn_blocking(move || git_trailers::analyze(&snapshot, &history))
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result.map(Arc::new).map_err(|error| format!("{error:#}"))),
    )
}
