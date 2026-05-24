"""Authentication providers for CDF SDK."""

from __future__ import annotations

import time
from abc import ABC, abstractmethod
from typing import Any


class AuthProvider(ABC):
    """Base class for authentication providers."""

    @abstractmethod
    def get_headers(self) -> dict[str, str]:
        """Return headers to add to every request."""

    @abstractmethod
    def refresh(self) -> None:
        """Refresh authentication if needed."""


class ApiKeyAuth(AuthProvider):
    """Authenticate with a static API key."""

    def __init__(self, api_key: str):
        self.api_key = api_key

    def get_headers(self) -> dict[str, str]:
        return {"X-API-Key": self.api_key}

    def refresh(self) -> None:
        pass


class JwtAuth(AuthProvider):
    """Authenticate with JWT tokens that auto-refresh."""

    def __init__(
        self,
        username: str,
        password: str,
        token_url: str = "/v1/auth/token",
        client: Any | None = None,
    ):
        self.username = username
        self.password = password
        self.token_url = token_url
        self._client = client
        self._token: str | None = None
        self._refresh_token: str | None = None
        self._expires_at: float = 0.0

    def get_headers(self) -> dict[str, str]:
        if self._token is None or time.time() >= self._expires_at - 60:
            self.refresh()
        return {"Authorization": f"Bearer {self._token}"}

    def refresh(self) -> None:
        if self._client is None:
            raise RuntimeError("JWT auth requires client reference for refresh")

        resp = self._client._request(
            "POST",
            self.token_url,
            json={"username": self.username, "password": self.password},
            auth=None,  # Don't recurse
        )
        data = resp.json()
        self._token = data["access_token"]
        self._refresh_token = data.get("refresh_token")
        self._expires_at = time.time() + data.get("expires_in", 3600)
