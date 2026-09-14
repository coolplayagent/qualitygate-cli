//! Navigation contracts have no authority to change rule enforcement.

use serde::{Deserialize, Serialize};

/// Preserve the v1 scalar encoding while allowing multiple explicit memberships.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CategoryMembership {
    One(String),
    Many(Vec<String>),
}

impl CategoryMembership {
    pub fn names(&self) -> &[String] {
        match self {
            Self::One(name) => std::slice::from_ref(name),
            Self::Many(names) => names,
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names().iter().any(|current| current == name)
    }
}

impl From<&str> for CategoryMembership {
    fn from(name: &str) -> Self {
        Self::One(name.into())
    }
}

impl From<String> for CategoryMembership {
    fn from(name: String) -> Self {
        Self::One(name)
    }
}
