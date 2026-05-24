"""CDF SDK exceptions."""

class CdfError(Exception):
    """Base exception for all CDF errors."""
    def __init__(self, message: str, status_code: int | None = None):
        self.message = message
        self.status_code = status_code
        super().__init__(message)

class CdfAuthError(CdfError):
    """Authentication or authorization failure."""
    pass

class CdfConnectionError(CdfError):
    """Unable to connect to CDF cluster."""
    pass

class CdfNotFoundError(CdfError):
    """Requested resource not found."""
    pass

class CdfPermissionError(CdfError):
    """Insufficient permissions for operation."""
    pass

class CdfValidationError(CdfError):
    """Invalid request data."""
    pass
