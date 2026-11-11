//! Pure accounting for namespace-scope expansion and name lookup work.
//! XML tokenization and tree parsing remain owned by the consuming profiles.

pub const NAMESPACE_WORK_LIMIT: usize = 10_000_000;
const LIMIT: &str = "XML namespace resolution exceeds 10000000 work units";

#[derive(Debug)]
pub struct NamespaceBudget {
    scopes: Vec<usize>,
    remaining: usize,
}

impl Default for NamespaceBudget {
    fn default() -> Self {
        Self {
            scopes: Vec::new(),
            remaining: NAMESPACE_WORK_LIMIT,
        }
    }
}

impl NamespaceBudget {
    /// Counts declarations conservatively, including shadowed prefixes. A new
    /// declaration can force pairwise inherited-prefix comparisons; names that
    /// need resolution can each scan the entire resulting scope.
    pub fn open(&mut self, declarations: usize, lookups: usize) -> Result<(), &'static str> {
        if self.scopes.len() >= 32 {
            return Err("XML namespace nesting exceeds 32 elements");
        }
        let parent = self.scopes.last().copied().unwrap_or_default();
        let scope = parent.checked_add(declarations).ok_or(LIMIT)?;
        let inherited = if declarations == 0 {
            0
        } else {
            parent.checked_mul(scope).ok_or(LIMIT)?
        };
        let work = inherited
            .checked_add(scope.checked_mul(lookups).ok_or(LIMIT)?)
            .ok_or(LIMIT)?;
        let remaining = self.remaining.checked_sub(work).ok_or(LIMIT)?;
        self.scopes.push(scope);
        self.remaining = remaining;
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), &'static str> {
        self.scopes
            .pop()
            .map(|_| ())
            .ok_or("Unbalanced XML namespace scopes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_work_is_charged_before_admission_and_not_reset_by_siblings() {
        let mut budget = NamespaceBudget {
            scopes: Vec::new(),
            remaining: 19,
        };
        budget.open(3, 1).unwrap();
        assert_eq!(budget.remaining, 16);
        budget.open(1, 1).unwrap();
        assert_eq!(budget.remaining, 0);
        budget.close().unwrap();
        // Reusing a scope without resolving a name needs no expansion work.
        budget.open(0, 0).unwrap();
        budget.close().unwrap();
        assert_eq!(budget.open(0, 1), Err(LIMIT));
        assert_eq!(budget.remaining, 0);
        assert_eq!(budget.scopes, [3]);
        budget.close().unwrap();
        assert!(budget.close().is_err());
    }

    #[test]
    fn namespace_budget_checks_arithmetic_and_bounds_its_scope_stack() {
        let mut budget = NamespaceBudget::default();
        assert_eq!(budget.open(usize::MAX, 2), Err(LIMIT));
        assert!(budget.scopes.is_empty());
        assert_eq!(budget.remaining, NAMESPACE_WORK_LIMIT);
        budget.open(1, 1).unwrap();
        let remaining = budget.remaining;
        for (declarations, lookups) in [(usize::MAX, 0), (1, usize::MAX), (0, usize::MAX)] {
            assert_eq!(budget.open(declarations, lookups), Err(LIMIT));
            assert_eq!(budget.scopes, [1]);
            assert_eq!(budget.remaining, remaining);
        }
        budget.close().unwrap();
        for _ in 0..32 {
            budget.open(0, 1).unwrap();
        }
        assert!(budget.open(0, 1).is_err());
        assert_eq!(budget.scopes.len(), 32);
        for _ in 0..32 {
            budget.close().unwrap();
        }
        assert!(budget.scopes.is_empty());
    }
}
