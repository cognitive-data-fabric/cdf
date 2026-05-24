//! Access Control List — fine-grained permissions on resources.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Actions that can be performed on resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Create, Read, Update, Delete, Admin,
    Search, Insert, DeleteIndex,
    Traverse, ReadEdges, WriteEdges,
    Alter, Evolve, Register,
    NodeJoin, ShardRebalance, Backup,
    All,
}

/// Resource types with identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum Resource {
    Table { namespace: String, name: String },
    VectorIndex { namespace: String, name: String },
    Graph { namespace: String, name: String },
    Schema { namespace: String, name: String },
    Cluster { node_id: String },
    Namespace(String),
    System,
}

impl Resource {
    pub fn namespace(&self) -> Option<&str> {
        match self {
            Resource::Table { namespace, .. } => Some(namespace),
            Resource::VectorIndex { namespace, .. } => Some(namespace),
            Resource::Graph { namespace, .. } => Some(namespace),
            Resource::Schema { namespace, .. } => Some(namespace),
            Resource::Namespace(ns) => Some(ns),
            _ => None,
        }
    }
}

/// One ACL entry granting specific actions on a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclEntry {
    pub principal_id: String,
    pub resource: Resource,
    pub actions: HashSet<Action>,
    pub granted_by: String,
    pub granted_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub conditions: Option<String>, // JSON for conditions like IP ranges
}

/// Permission matrix for a principal.
#[derive(Debug, Clone, Default)]
pub struct Permission {
    pub table_perms: HashMap<String, HashSet<Action>>,
    pub vector_perms: HashMap<String, HashSet<Action>>,
    pub graph_perms: HashMap<String, HashSet<Action>>,
    pub schema_perms: HashMap<String, HashSet<Action>>,
    pub cluster_perms: HashSet<Action>,
    pub namespace_perms: HashMap<String, HashSet<Action>>,
    pub system_admin: bool,
}

/// ACL manager for checking and granting permissions.
pub struct AclManager {
    entries: Vec<AclEntry>,
}

impl AclManager {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn grant(&mut self, entry: AclEntry) {
        self.entries.push(entry);
    }

    pub fn revoke(&mut self, principal_id: &str, resource: &Resource, action: Action) {
        self.entries.retain(|e| {
            !(e.principal_id == principal_id
                && e.resource == *resource
                && e.actions.contains(&action))
        });
    }

    pub fn check(
        &self,
        principal_id: &str,
        action: Action,
        resource: &Resource,
    ) -> bool {
        let now = chrono::Utc::now();
        self.entries.iter().any(|e| {
            e.principal_id == principal_id
                && e.actions.contains(&action)
                && e.resource == *resource
                && e.expires_at.map_or(true, |exp| now < exp)
        })
    }

    pub fn check_any(
        &self,
        principal_id: &str,
        actions: &[Action],
        resource: &Resource,
    ) -> bool {
        actions.iter().any(|a| self.check(principal_id, *a, resource))
    }

    /// Build a permission summary for a principal.
    pub fn build_permissions(&self, principal_id: &str) -> Permission {
        let mut perm = Permission::default();
        for e in &self.entries {
            if e.principal_id != principal_id {
                continue;
            }
            if let Some(exp) = e.expires_at {
                if chrono::Utc::now() >= exp {
                    continue;
                }
            }
            match &e.resource {
                Resource::Table { namespace: _, name } => {
                    perm.table_perms.entry(name.clone()).or_default().extend(&e.actions);
                }
                Resource::VectorIndex { namespace: _, name } => {
                    perm.vector_perms.entry(name.clone()).or_default().extend(&e.actions);
                }
                Resource::Graph { namespace: _, name } => {
                    perm.graph_perms.entry(name.clone()).or_default().extend(&e.actions);
                }
                Resource::Schema { namespace: _, name } => {
                    perm.schema_perms.entry(name.clone()).or_default().extend(&e.actions);
                }
                Resource::Cluster { .. } => {
                    perm.cluster_perms.extend(&e.actions);
                }
                Resource::Namespace(ns) => {
                    perm.namespace_perms.entry(ns.clone()).or_default().extend(&e.actions);
                }
                Resource::System => {
                    if e.actions.contains(&Action::Admin) {
                        perm.system_admin = true;
                    }
                }
            }
        }
        perm
    }
}

impl Default for AclManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_grant_revoke() {
        let mut acl = AclManager::new();
        let entry = AclEntry {
            principal_id: "user1".to_string(),
            resource: Resource::Table { namespace: "default".into(), name: "docs".into() },
            actions: [Action::Read, Action::Create].into_iter().collect(),
            granted_by: "admin".into(),
            granted_at: chrono::Utc::now(),
            expires_at: None,
            conditions: None,
        };
        acl.grant(entry);

        assert!(acl.check("user1", Action::Read, &Resource::Table { namespace: "default".into(), name: "docs".into() }));
        assert!(!acl.check("user1", Action::Delete, &Resource::Table { namespace: "default".into(), name: "docs".into() }));

        acl.revoke("user1", &Resource::Table { namespace: "default".into(), name: "docs".into() }, Action::Read);
        assert!(!acl.check("user1", Action::Read, &Resource::Table { namespace: "default".into(), name: "docs".into() }));
    }

    #[test]
    fn test_expired_acl() {
        let mut acl = AclManager::new();
        let entry = AclEntry {
            principal_id: "user1".to_string(),
            resource: Resource::Table { namespace: "default".into(), name: "docs".into() },
            actions: [Action::Read].into_iter().collect(),
            granted_by: "admin".into(),
            granted_at: chrono::Utc::now(),
            expires_at: Some(chrono::Utc::now() - chrono::Duration::hours(1)),
            conditions: None,
        };
        acl.grant(entry);
        assert!(!acl.check("user1", Action::Read, &Resource::Table { namespace: "default".into(), name: "docs".into() }));
    }
}
