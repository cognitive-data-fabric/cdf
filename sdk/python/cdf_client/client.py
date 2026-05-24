"""CDF Client — main entry point for data operations."""

from __future__ import annotations

import json
from typing import Any

import requests

from .auth import ApiKeyAuth, AuthProvider
from .exceptions import (
    CdfAuthError,
    CdfConnectionError,
    CdfError,
    CdfNotFoundError,
    CdfPermissionError,
    CdfValidationError,
)


class CdfClient:
    """Client for Cognitive Data Fabric.

    Usage:
        from cdf_client import CdfClient, ApiKeyAuth

        client = CdfClient("http://localhost:8080", auth=ApiKeyAuth("key-123"))
        client.insert("documents", {"title": "Hello", "content": "World"})
        results = client.search("documents", vector=[0.1, 0.2, ...], top_k=5)
    """

    def __init__(
        self,
        base_url: str,
        auth: AuthProvider | None = None,
        timeout: float = 30.0,
        retries: int = 3,
    ):
        self.base_url = base_url.rstrip("/")
        self.auth = auth or ApiKeyAuth("")
        self.timeout = timeout
        self.retries = retries
        self.session = requests.Session()

    def _request(
        self,
        method: str,
        path: str,
        **kwargs: Any,
    ) -> requests.Response:
        url = f"{self.base_url}{path}"
        headers = kwargs.pop("headers", {})
        headers.update(self.auth.get_headers())
        headers.setdefault("Content-Type", "application/json")

        last_error: Exception | None = None
        for attempt in range(self.retries):
            try:
                resp = self.session.request(
                    method, url, headers=headers, timeout=self.timeout, **kwargs
                )
                self._raise_for_status(resp)
                return resp
            except requests.ConnectionError as e:
                last_error = CdfConnectionError(str(e))
            except requests.Timeout as e:
                last_error = CdfConnectionError(f"Timeout: {e}")
        raise last_error or CdfConnectionError("Max retries exceeded")

    def _raise_for_status(self, resp: requests.Response) -> None:
        if resp.status_code < 400:
            return
        try:
            data = resp.json()
            msg = data.get("message", data.get("error", resp.text))
        except Exception:
            msg = resp.text

        if resp.status_code == 401:
            raise CdfAuthError(msg, resp.status_code)
        elif resp.status_code == 403:
            raise CdfPermissionError(msg, resp.status_code)
        elif resp.status_code == 404:
            raise CdfNotFoundError(msg, resp.status_code)
        elif resp.status_code == 422:
            raise CdfValidationError(msg, resp.status_code)
        else:
            raise CdfError(msg, resp.status_code)

    # --- Health ---

    def health(self) -> dict[str, Any]:
        """Check cluster health."""
        resp = self._request("GET", "/health")
        return resp.json()

    # --- Table Operations ---

    def create_table(
        self,
        name: str,
        schema: dict[str, Any] | None = None,
        namespace: str = "default",
    ) -> dict[str, Any]:
        """Create a new table."""
        payload = {"name": name, "namespace": namespace}
        if schema:
            payload["schema"] = schema
        resp = self._request("POST", "/v1/tables", json=payload)
        return resp.json()

    def list_tables(self, namespace: str = "default") -> list[dict[str, Any]]:
        """List tables in a namespace."""
        resp = self._request("GET", f"/v1/tables?namespace={namespace}")
        return resp.json().get("tables", [])

    def get_table(self, name: str, namespace: str = "default") -> dict[str, Any]:
        """Get table metadata."""
        resp = self._request("GET", f"/v1/tables/{namespace}/{name}")
        return resp.json()

    def drop_table(self, name: str, namespace: str = "default") -> None:
        """Drop a table."""
        self._request("DELETE", f"/v1/tables/{namespace}/{name}")

    # --- CRUD ---

    def insert(
        self,
        table: str,
        data: dict[str, Any],
        namespace: str = "default",
    ) -> dict[str, Any]:
        """Insert a row."""
        payload = {"namespace": namespace, "table": table, "data": data}
        resp = self._request("POST", "/v1/insert", json=payload)
        return resp.json()

    def batch_insert(
        self,
        table: str,
        rows: list[dict[str, Any]],
        namespace: str = "default",
    ) -> dict[str, Any]:
        """Insert multiple rows."""
        payload = {"namespace": namespace, "table": table, "rows": rows}
        resp = self._request("POST", "/v1/batch_insert", json=payload)
        return resp.json()

    def get(
        self,
        table: str,
        row_id: str,
        namespace: str = "default",
    ) -> dict[str, Any] | None:
        """Get a row by ID."""
        try:
            resp = self._request(
                "GET", f"/v1/rows/{namespace}/{table}/{row_id}"
            )
            return resp.json()
        except CdfNotFoundError:
            return None

    def update(
        self,
        table: str,
        row_id: str,
        data: dict[str, Any],
        namespace: str = "default",
    ) -> dict[str, Any]:
        """Update a row."""
        payload = {"namespace": namespace, "table": table, "row_id": row_id, "data": data}
        resp = self._request("PUT", "/v1/update", json=payload)
        return resp.json()

    def delete(
        self,
        table: str,
        row_id: str,
        namespace: str = "default",
    ) -> None:
        """Delete a row."""
        self._request(
            "DELETE", f"/v1/rows/{namespace}/{table}/{row_id}"
        )

    # --- Query ---

    def query(self, cql: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        """Execute a CQL query."""
        payload = {"query": cql}
        if params:
            payload["params"] = params
        resp = self._request("POST", "/v1/query", json=payload)
        return resp.json()

    # --- Vector Search ---

    def search(
        self,
        table: str,
        vector: list[float],
        top_k: int = 10,
        threshold: float | None = None,
        namespace: str = "default",
        filter: dict[str, Any] | None = None,
    ) -> list[dict[str, Any]]:
        """Semantic vector search."""
        payload = {
            "namespace": namespace,
            "table": table,
            "vector": vector,
            "top_k": top_k,
        }
        if threshold is not None:
            payload["threshold"] = threshold
        if filter:
            payload["filter"] = filter
        resp = self._request("POST", "/v1/search", json=payload)
        return resp.json().get("results", [])

    def search_text(
        self,
        table: str,
        text: str,
        top_k: int = 10,
        namespace: str = "default",
    ) -> list[dict[str, Any]]:
        """Search by text (auto-embed)."""
        payload = {
            "namespace": namespace,
            "table": table,
            "text": text,
            "top_k": top_k,
        }
        resp = self._request("POST", "/v1/search_text", json=payload)
        return resp.json().get("results", [])

    # --- Graph ---

    def add_edge(
        self,
        from_id: str,
        to_id: str,
        edge_type: str,
        properties: dict[str, Any] | None = None,
        namespace: str = "default",
    ) -> dict[str, Any]:
        """Add a graph edge."""
        payload = {
            "namespace": namespace,
            "from_id": from_id,
            "to_id": to_id,
            "edge_type": edge_type,
        }
        if properties:
            payload["properties"] = properties
        resp = self._request("POST", "/v1/graph/edges", json=payload)
        return resp.json()

    def traverse(
        self,
        start_id: str,
        edge_types: list[str] | None = None,
        depth: int = 2,
        namespace: str = "default",
    ) -> list[dict[str, Any]]:
        """Graph traversal (BFS)."""
        params: dict[str, Any] = {
            "start_id": start_id,
            "depth": depth,
            "namespace": namespace,
        }
        if edge_types:
            params["edge_types"] = edge_types
        resp = self._request("GET", "/v1/graph/traverse", params=params)
        return resp.json().get("paths", [])

    def neighbors(
        self,
        node_id: str,
        edge_type: str | None = None,
        namespace: str = "default",
    ) -> list[dict[str, Any]]:
        """Get immediate neighbors."""
        params: dict[str, Any] = {"node_id": node_id, "namespace": namespace}
        if edge_type:
            params["edge_type"] = edge_type
        resp = self._request("GET", "/v1/graph/neighbors", params=params)
        return resp.json().get("neighbors", [])

    # --- Streaming ---

    def subscribe(
        self,
        table: str,
        event_types: list[str] | None = None,
        namespace: str = "default",
    ):
        """Subscribe to table changes via SSE (returns iterator)."""
        import sseclient

        params: dict[str, Any] = {"namespace": namespace, "table": table}
        if event_types:
            params["events"] = ",".join(event_types)

        headers = self.auth.get_headers()
        headers["Accept"] = "text/event-stream"
        resp = requests.get(
            f"{self.base_url}/v1/subscribe",
            params=params,
            headers=headers,
            stream=True,
            timeout=None,
        )
        return sseclient.SSEClient(resp)

    # --- Admin (delegated) ---

    @property
    def admin(self) -> "CdfAdmin":
        """Access admin operations."""
        from .admin import CdfAdmin

        return CdfAdmin(self)
