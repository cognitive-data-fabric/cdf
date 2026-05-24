"""CDF Python SDK — Cognitive Data Fabric Client.

Provides typed access to CDF's REST and gRPC APIs with
authentication, connection pooling, and retry logic.
"""

from .client import CdfClient
from .auth import ApiKeyAuth, JwtAuth
from .admin import CdfAdmin
from .exceptions import (
    CdfError,
    CdfAuthError,
    CdfConnectionError,
    CdfNotFoundError,
    CdfPermissionError,
    CdfValidationError,
)

__version__ = "0.1.0"
__all__ = [
    "CdfClient",
    "ApiKeyAuth",
    "JwtAuth",
    "CdfAdmin",
    "CdfError",
    "CdfAuthError",
    "CdfConnectionError",
    "CdfNotFoundError",
    "CdfPermissionError",
    "CdfValidationError",
]
