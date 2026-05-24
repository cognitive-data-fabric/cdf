"""CDF Admin SDK — user, role, ACL, and cluster management."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .client import CdfClient


class CdfAdmin:
    """Admin operations for CDF: users, roles, ACL, namespaces, cluster."""

    def __init__(self, client: "CdfClient"):
        self._client = client

    # --- Authentication ---

    def login(self, username: str, password: str) -> dict[str, Any]:
        """Authenticate and get tokens."""
        resp = self._client._request(
            "POST",
            "/v1/auth/token",
            json={"username": username, "password": password},
        )
        return resp.json()

    def refresh_token(self, refresh_token: str) -> dict[str, Any]:
        """Refresh access token."""
        resp = self._client._request(
            "POST",
            "/v1/auth/refresh",
            json={"refresh_token": refresh_token},
        )
        return resp.json()

    def revoke_token(self, token_id: str) -> None:
        """Revoke a specific token."""
        self._client._request("POST", "/v1/auth/revoke", json={"token_id": token_id})

    # --- User Management ---

    def create_user(
        self,
        username: str,
        email: str,
        password: str,
        roles: list[str],
        namespaces: list[str] | None = None,
    ) -> dict[str, Any]:
        """Create a new user."""
        payload = {
            "username": username,
            "email": email,
            "password": password,
            "roles": roles,
            "namespaces": namespaces or ["default"],
        }
        resp = self._client._request("POST", "/v1/admin/users", json=payload)
        return resp.json()

    def list_users(
        self,
        namespace: str | None = None,
        role: str | None = None,
    ) -> list[dict[str, Any]]:
        """List users with optional filtering."""
        params: dict[str, Any] = {}
        if namespace:
            params["namespace"] = namespace
        if role:
            params["role"] = role
        resp = self._client._request("GET", "/v1/admin/users", params=params)
        return resp.json().get("users", [])

    def get_user(self, user_id: str) -> dict[str, Any]:
        """Get user details."""
        resp = self._client._request("GET", f"/v1/admin/users/{user_id}")
        return resp.json()

    def update_user(
        self,
        user_id: str,
        roles: list[str] | None = None,
        namespaces: list[str] | None = None,
        disabled: bool | None = None,
        mfa_enabled: bool | None = None,
    ) -> dict[str, Any]:
        """Update user attributes."""
        payload: dict[str, Any] = {}
        if roles is not None:
            payload["roles"] = roles
        if namespaces is not None:
            payload["namespaces"] = namespaces
        if disabled is not None:
            payload["disabled"] = disabled
        if mfa_enabled is not None:
            payload["mfa_enabled"] = mfa_enabled
        resp = self._client._request(
            "PUT", f"/v1/admin/users/{user_id}", json=payload
        )
        return resp.json()

    def delete_user(self, user_id: str) -> None:
        """Delete a user."""
        self._client._request("DELETE", f"/v1/admin/users/{user_id}")

    def reset_password(self, user_id: str, new_password: str) -> None:
        """Reset user password."""
        self._client._request(
            "POST",
            f"/v1/admin/users/{user_id}/reset_password",
            json={"password": new_password},
        )

    # --- API Keys ---

    def create_api_key(
        self,
        name: str,
        user_id: str,
        roles: list[str],
        expires_in_days: int = 365,
    ) -> dict[str, Any]:
        """Create an API key for a user."""
        payload = {
            "name": name,
            "user_id": user_id,
            "roles": roles,
            "expires_in_days": expires_in_days,
        }
        resp = self._client._request("POST", "/v1/admin/apikeys", json=payload)
        return resp.json()

    def list_api_keys(self, user_id: str | None = None) -> list[dict[str, Any]]:
        """List API keys."""
        params: dict[str, Any] = {}
        if user_id:
            params["user_id"] = user_id
        resp = self._client._request("GET", "/v1/admin/apikeys", params=params)
        return resp.json().get("keys", [])

    def revoke_api_key(self, key_id: str) -> None:
        """Revoke an API key."""
        self._client._request("DELETE", f"/v1/admin/apikeys/{key_id}")

    # --- ACL Management ---

    def grant_acl(
        self,
        principal_id: str,
        resource_type: str,
        resource_id: str,
        actions: list[str],
        namespace: str = "default",
        expires_at: str | None = None,
        conditions: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Grant ACL permissions."""
        payload = {
            "principal_id": principal_id,
            "resource_type": resource_type,
            "resource_id": resource_id,
            "actions": actions,
            "namespace": namespace,
        }
        if expires_at:
            payload["expires_at"] = expires_at
        if conditions:
            payload["conditions"] = conditions
        resp = self._client._request("POST", "/v1/admin/acl", json=payload)
        return resp.json()

    def revoke_acl(
        self,
        principal_id: str,
        resource_type: str,
        resource_id: str,
        actions: list[str] | None = None,
        namespace: str = "default",
    ) -> None:
        """Revoke ACL permissions."""
        payload: dict[str, Any] = {
            "principal_id": principal_id,
            "resource_type": resource_type,
            "resource_id": resource_id,
            "namespace": namespace,
        }
        if actions:
            payload["actions"] = actions
        self._client._request("DELETE", "/v1/admin/acl", json=payload)

    def list_acl(
        self,
        principal_id: str | None = None,
        resource_type: str | None = None,
        namespace: str | None = None,
    ) -> list[dict[str, Any]]:
        """List ACL entries."""
        params: dict[str, Any] = {}
        if principal_id:
            params["principal_id"] = principal_id
        if resource_type:
            params["resource_type"] = resource_type
        if namespace:
            params["namespace"] = namespace
        resp = self._client._request("GET", "/v1/admin/acl", params=params)
        return resp.json().get("entries", [])

    def get_permissions(self, principal_id: str) -> dict[str, Any]:
        """Get effective permission matrix for a principal."""
        resp = self._client._request(
            "GET", f"/v1/admin/permissions/{principal_id}"
        )
        return resp.json()

    # --- Namespace Management ---

    def create_namespace(self, name: str, owner: str | None = None) -> dict[str, Any]:
        """Create a namespace."""
        payload: dict[str, Any] = {"name": name}
        if owner:
            payload["owner"] = owner
        resp = self._client._request("POST", "/v1/admin/namespaces", json=payload)
        return resp.json()

    def list_namespaces(self) -> list[dict[str, Any]]:
        """List all namespaces."""
        resp = self._client._request("GET", "/v1/admin/namespaces")
        return resp.json().get("namespaces", [])

    def get_namespace(self, name: str) -> dict[str, Any]:
        """Get namespace details."""
        resp = self._client._request("GET", f"/v1/admin/namespaces/{name}")
        return resp.json()

    def update_namespace(
        self,
        name: str,
        owner: str | None = None,
        quotas: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Update namespace settings."""
        payload: dict[str, Any] = {}
        if owner:
            payload["owner"] = owner
        if quotas:
            payload["quotas"] = quotas
        resp = self._client._request(
            "PUT", f"/v1/admin/namespaces/{name}", json=payload
        )
        return resp.json()

    def delete_namespace(self, name: str, force: bool = False) -> None:
        """Delete a namespace."""
        params = {"force": "true" if force else "false"}
        self._client._request(
            "DELETE", f"/v1/admin/namespaces/{name}", params=params
        )

    # --- Cluster Management ---

    def cluster_status(self) -> dict[str, Any]:
        """Get cluster health and node status."""
        resp = self._client._request("GET", "/v1/admin/cluster/status")
        return resp.json()

    def list_nodes(self) -> list[dict[str, Any]]:
        """List cluster nodes."""
        resp = self._client._request("GET", "/v1/admin/cluster/nodes")
        return resp.json().get("nodes", [])

    def drain_node(self, node_id: str) -> None:
        """Drain a node for maintenance."""
        self._client._request(
            "POST", f"/v1/admin/cluster/nodes/{node_id}/drain"
        )

    def activate_node(self, node_id: str) -> None:
        """Reactivate a drained node."""
        self._client._request(
            "POST", f"/v1/admin/cluster/nodes/{node_id}/activate"
        )

    def rebalance_shards(self) -> dict[str, Any]:
        """Trigger shard rebalancing."""
        resp = self._client._request("POST", "/v1/admin/cluster/rebalance")
        return resp.json()

    # --- Backups ---

    def create_backup(self, name: str, tables: list[str] | None = None) -> dict[str, Any]:
        """Create a backup."""
        payload: dict[str, Any] = {"name": name}
        if tables:
            payload["tables"] = tables
        resp = self._client._request("POST", "/v1/admin/backups", json=payload)
        return resp.json()

    def list_backups(self) -> list[dict[str, Any]]:
        """List backups."""
        resp = self._client._request("GET", "/v1/admin/backups")
        return resp.json().get("backups", [])

    def restore_backup(self, backup_id: str, target_namespace: str | None = None) -> dict[str, Any]:
        """Restore from backup."""
        payload: dict[str, Any] = {"backup_id": backup_id}
        if target_namespace:
            payload["target_namespace"] = target_namespace
        resp = self._client._request("POST", "/v1/admin/backups/restore", json=payload)
        return resp.json()

    def delete_backup(self, backup_id: str) -> None:
        """Delete a backup."""
        self._client._request("DELETE", f"/v1/admin/backups/{backup_id}")

    # --- Audit Logs ---

    def list_audit_logs(
        self,
        principal_id: str | None = None,
        action: str | None = None,
        resource: str | None = None,
        start_time: str | None = None,
        end_time: str | None = None,
        limit: int = 100,
    ) -> list[dict[str, Any]]:
        """Query audit logs."""
        params: dict[str, Any] = {"limit": limit}
        if principal_id:
            params["principal_id"] = principal_id
        if action:
            params["action"] = action
        if resource:
            params["resource"] = resource
        if start_time:
            params["start_time"] = start_time
        if end_time:
            params["end_time"] = end_time
        resp = self._client._request("GET", "/v1/admin/audit", params=params)
        return resp.json().get("entries", [])

    # --- Schema Registry ---

    def register_schema(self, name: str, schema: dict[str, Any], namespace: str = "default") -> dict[str, Any]:
        """Register a schema."""
        payload = {
            "name": name,
            "namespace": namespace,
            "schema": schema,
        }
        resp = self._client._request("POST", "/v1/admin/schemas", json=payload)
        return resp.json()

    def get_schema(self, name: str, namespace: str = "default", version: int | None = None) -> dict[str, Any]:
        """Get schema by name and optional version."""
        params: dict[str, Any] = {}
        if version:
            params["version"] = version
        resp = self._client._request(
            "GET", f"/v1/admin/schemas/{namespace}/{name}", params=params
        )
        return resp.json()

    def list_schemas(self, namespace: str = "default") -> list[dict[str, Any]]:
        """List schemas in namespace."""
        resp = self._client._request(
            "GET", f"/v1/admin/schemas?namespace={namespace}"
        )
        return resp.json().get("schemas", [])

    def evolve_schema(
        self,
        name: str,
        changes: list[dict[str, Any]],
        namespace: str = "default",
    ) -> dict[str, Any]:
        """Evolve a schema (add/remove columns)."""
        payload = {
            "namespace": namespace,
            "name": name,
            "changes": changes,
        }
        resp = self._client._request("POST", "/v1/admin/schemas/evolve", json=payload)
        return resp.json()
