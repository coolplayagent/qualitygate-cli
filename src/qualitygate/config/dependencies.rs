//! Indexed dependency validation and stable iterative graph traversal.
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Graph<'a> {
    ids: Vec<&'a str>,
    index: BTreeMap<&'a str, usize>,
    prerequisites: Vec<Vec<usize>>,
    consumers: Vec<Vec<usize>>,
}

impl<'a> Graph<'a> {
    pub(super) fn new(nodes: impl IntoIterator<Item = (&'a str, &'a [String])>) -> Result<Self> {
        let nodes: Vec<_> = nodes.into_iter().collect();
        let mut index = BTreeMap::new();
        for (position, (id, _)) in nodes.iter().enumerate() {
            if index.insert(*id, position).is_some() {
                bail!("Duplicate check id: {id}");
            }
        }
        let mut prerequisites = vec![Vec::new(); nodes.len()];
        let mut consumers = vec![Vec::new(); nodes.len()];
        for (position, (_, dependencies)) in nodes.iter().enumerate() {
            let mut unique = BTreeSet::new();
            for dependency in *dependencies {
                let Some(&source) = index.get(dependency.as_str()) else {
                    bail!("Unknown prerequisite: {dependency}");
                };
                if !unique.insert(source) {
                    bail!("Repeated prerequisite: {dependency}");
                }
                prerequisites[position].push(source);
                consumers[source].push(position);
            }
        }
        Ok(Self {
            ids: nodes.iter().map(|(id, _)| *id).collect(),
            index,
            prerequisites,
            consumers,
        })
    }

    /// Resolve each reachable vertex once, without recursion or whole-graph rescans.
    pub(super) fn closure(&self, roots: &BTreeSet<String>) -> Result<BTreeSet<String>> {
        let mut visited = vec![false; self.ids.len()];
        let mut pending = Vec::new();
        for root in roots {
            let Some(&index) = self.index.get(root.as_str()) else {
                bail!("Unknown check: {root}");
            };
            visited[index] = true;
            pending.push(index);
        }
        while let Some(index) = pending.pop() {
            for &dependency in &self.prerequisites[index] {
                if !visited[dependency] {
                    visited[dependency] = true;
                    pending.push(dependency);
                }
            }
        }
        Ok(self
            .ids
            .iter()
            .zip(visited)
            .filter(|(_, visited)| *visited)
            .map(|(id, _)| (*id).to_owned())
            .collect())
    }

    /// Choose the earliest originally declared eligible node, preserving plan order.
    pub(super) fn order(&self) -> Result<Vec<String>> {
        let mut remaining: Vec<_> = self.prerequisites.iter().map(Vec::len).collect();
        let mut ready: BTreeSet<_> = remaining
            .iter()
            .enumerate()
            .filter_map(|(index, count)| (*count == 0).then_some(index))
            .collect();
        let mut order = Vec::with_capacity(self.ids.len());
        while let Some(index) = ready.pop_first() {
            order.push(self.ids[index].to_owned());
            for &consumer in &self.consumers[index] {
                remaining[consumer] -= 1;
                if remaining[consumer] == 0 {
                    ready.insert(consumer);
                }
            }
        }
        if order.len() != self.ids.len() {
            bail!("Check dependency graph contains a cycle");
        }
        Ok(order)
    }
}

#[cfg(test)]
#[path = "dependencies_tests.rs"]
mod tests;
