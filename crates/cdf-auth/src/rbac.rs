//! Role-Based Access Control definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Built-in roles with hierarchical permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full system access — can manage users, namespaces, clusters.
    SuperAdmin,
    /// Namespace management — create/drop tables, manage users within namespace.
    Admin,
    /// Application developer — CRUD on tables, create indexes.
    Developer,
    /// Read-only analyst — query tables, search vectors.
    Analyst,
    /// Service account — service-to-service auth, no human login.
    Service,
}

impl Role {
    /// Returns the rank (higher = more permissions).
    pub fn rank(&self) -> u8 {
        match self {
            Role::SuperAdmin => 100,
            Role::Admin => 80,
            Role::Developer => 60,
            Role::Analyst => 40,
            Role::Service => 20,
        }
    }

    /// Returns implied roles (e.g., Admin implies Developer).
    pub fn implied_roles(&self) -> Vec<Role> {
        match self {
            Role::SuperAdmin => vec![
                Role::SuperAdmin,
                Role::Admin,
                Role::Developer,
                Role::Analyst,
                Role::Service,
            ],
            Role::Admin => vec![Role::Admin, Role::Developer, Role::Analyst],
            Role::Developer => vec![Role::Developer, Role::Analyst],
            Role::Analyst => vec![Role::Analyst],
            Role::Service => vec![Role::Service],
        }
    }

    /// Check if this role satisfies a required role (considers hierarchy).
    pub fn satisfies(&self, required: &Role) -> bool {
        self.implied_roles().contains(required)
    }
}

/// User record stored in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub roles: Vec<Role>,
    pub namespaces: Vec<String>,
    pub api_keys: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub password_hash: Option<String>,
}

/// Role manager for querying role hierarchies and permissions.
pub struct RoleManager {
    custom_roles: HashMap<String, Vec<Role>>,
}

impl RoleManager {
    pub fn new() -> Self {
        Self {
            custom_roles: HashMap::new(),
        }
    }

    pub fn add_custom_role(&mut self, name: String, base_roles: Vec<Role>) {
        self.custom_roles.insert(name, base_roles);
    }

    pub fn resolve_roles(&self, roles: &[Role]) -> Vec<Role> {
        let mut resolved = HashMap::new();
        for role in roles {
            for implied in role.implied_roles() {
                resolved.insert(implied, ());
            }
        }
        resolved.into_keys().collect()
    }
}

impl Default for RoleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_hierarchy() {
        assert!(Role::SuperAdmin.satisfies(&Role::Analyst));
        assert!(Role::Admin.satisfies(&Role::Developer));
        assert!(!Role::Analyst.satisfies(&Role::Developer));
    }

    #[test]
    fn test_role_rank() {
        assert!(Role::SuperAdmin.rank() > Role::Admin.rank());
        assert!(Role::Admin.rank() > Role::Developer.rank());
    }
}
