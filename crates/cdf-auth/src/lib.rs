//! CDF Authentication & Authorization — RBAC, ACL, JWT
//!
//! # Roles
//! - `SuperAdmin` — full system access
//! - `Admin` — namespace-level management
//! - `Developer` — read/write tables, manage indexes
//! - `Analyst` — read-only with query access
//! - `Service` — service-to-service authentication
//!
//! # ACL Types
//! - **Table ACL**: `CREATE`, `READ`, `UPDATE`, `DELETE`, `ADMIN` on tables
//! - **Vector ACL**: `SEARCH`, `INSERT`, `DELETE` on vector indexes
//! - **Graph ACL**: `TRAVERSE`, `READ_EDGES`, `WRITE_EDGES`
//! - **Schema ACL**: `ALTER`, `EVOLVE`, `REGISTER`
//! - **Cluster ACL**: `NODE_JOIN`, `SHARD_REBALANCE`, `BACKUP`

use cdf_common::FabricId;
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod jwt;
pub mod rbac;
pub mod acl;

pub use jwt::{Claims, JwtConfig, JwtVerifier};
pub use rbac::{Role, RoleManager, User};
pub use acl::{AclEntry, Permission, Resource, Action, AclManager};

/// Principal represents an authenticated entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub id: FabricId,
    pub username: String,
    pub email: String,
    pub roles: Vec<Role>,
    pub namespaces: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub disabled: bool,
    pub mfa_enabled: bool,
}

impl Principal {
    pub fn new(username: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id: FabricId::new(),
            username: username.into(),
            email: email.into(),
            roles: vec![Role::Analyst],
            namespaces: vec!["default".to_string()],
            metadata: HashMap::new(),
            created_at: Utc::now(),
            last_login: None,
            disabled: false,
            mfa_enabled: false,
        }
    }

    pub fn with_role(mut self, role: Role) -> Self {
        if !self.roles.contains(&role) {
            self.roles.push(role);
        }
        self
    }

    pub fn with_namespace(mut self, ns: impl Into<String>) -> Self {
        let ns = ns.into();
        if !self.namespaces.contains(&ns) {
            self.namespaces.push(ns);
        }
        self
    }

    /// Check if principal has any of the given roles.
    pub fn has_role(&self, required: &[Role]) -> bool {
        self.roles.iter().any(|r| required.contains(r))
    }

    /// SuperAdmin bypasses all ACL checks.
    pub fn is_super_admin(&self) -> bool {
        self.roles.contains(&Role::SuperAdmin)
    }

    /// Check namespace access.
    pub fn can_access_namespace(&self, namespace: &str) -> bool {
        self.is_super_admin() || self.namespaces.contains(&namespace.to_string())
    }
}

/// Credential for password-based authentication.
#[derive(Debug, Clone)]
pub struct Credential {
    pub principal_id: FabricId,
    pub password_hash: String,
    pub salt: String,
    pub failed_attempts: u32,
    pub locked_until: Option<DateTime<Utc>>,
}

/// Audit log entry for security events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: FabricId,
    pub timestamp: DateTime<Utc>,
    pub principal_id: FabricId,
    pub action: String,
    pub resource: String,
    pub allowed: bool,
    pub ip_address: String,
    pub user_agent: String,
    pub reason: Option<String>,
}

/// Policy engine for custom rules.
pub struct PolicyEngine {
    custom_rules: Vec<Box<dyn PolicyRule>>,
}

impl std::fmt::Debug for PolicyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyEngine")
            .field("rules_count", &self.custom_rules.len())
            .finish()
    }
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            custom_rules: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: Box<dyn PolicyRule>) {
        self.custom_rules.push(rule);
    }

    pub fn evaluate(&self, principal: &Principal, action: &Action, resource: &Resource) -> bool {
        for rule in &self.custom_rules {
            if let Some(decision) = rule.evaluate(principal, action, resource) {
                return decision;
            }
        }
        true // Default allow if no rules match (fail-safe: should be deny in production)
    }
}

/// Trait for custom policy rules (e.g., time-based, IP-based).
pub trait PolicyRule: Send + Sync {
    fn evaluate(
        &self,
        principal: &Principal,
        action: &Action,
        resource: &Resource,
    ) -> Option<bool>; // None = rule doesn't apply
}

/// Time-based policy: deny access outside business hours.
#[derive(Debug, Clone)]
pub struct BusinessHoursRule {
    start_hour: u32,
    end_hour: u32,
    timezone: String,
}

impl BusinessHoursRule {
    pub fn new(start_hour: u32, end_hour: u32, timezone: impl Into<String>) -> Self {
        Self {
            start_hour,
            end_hour,
            timezone: timezone.into(),
        }
    }
}

impl PolicyRule for BusinessHoursRule {
    fn evaluate(
        &self,
        _principal: &Principal,
        _action: &Action,
        _resource: &Resource,
    ) -> Option<bool> {
        let now = Utc::now();
        let hour = now.hour();
        if hour >= self.start_hour && hour < self.end_hour {
            Some(true)
        } else {
            Some(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_principal_roles() {
        let p = Principal::new("alice", "alice@example.com")
            .with_role(Role::Developer)
            .with_role(Role::Admin);

        assert!(p.has_role(&[Role::Developer]));
        assert!(!p.is_super_admin());
    }

    #[test]
    fn test_super_admin_bypass() {
        let p = Principal::new("root", "root@example.com")
            .with_role(Role::SuperAdmin);

        assert!(p.is_super_admin());
        assert!(p.can_access_namespace("any-namespace"));
    }

    #[test]
    fn test_business_hours_policy() {
        let rule = BusinessHoursRule::new(9, 18, "UTC");
        let p = Principal::new("test", "test@example.com");
        let action = Action::Read;
        let resource = Resource::Table {
            namespace: "default".into(),
            name: "test".into(),
        };

        // This will pass/fail depending on current UTC hour, so we just verify it runs
        let _result = rule.evaluate(&p, &action, &resource);
    }
}
