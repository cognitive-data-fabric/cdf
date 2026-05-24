export class ApiKeyAuth {
  constructor(apiKey) {
    this.apiKey = apiKey;
  }

  getHeaders() {
    return { "X-API-Key": this.apiKey };
  }

  refresh() {
    // No-op for static keys
  }
}

export class JwtAuth {
  constructor(username, password, tokenUrl = "/v1/auth/token", client) {
    this.username = username;
    this.password = password;
    this.tokenUrl = tokenUrl;
    this.client = client;
    this.token = null;
    this.refreshToken = null;
    this.expiresAt = 0;
  }

  getHeaders() {
    if (!this.token || Date.now() >= this.expiresAt - 60000) {
      this.refresh();
    }
    return { Authorization: `Bearer ${this.token}` };
  }

  refresh() {
    if (!this.client) {
      throw new Error("JWT auth requires client reference for refresh");
    }
  }
}
