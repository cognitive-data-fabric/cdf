export interface AuthProvider {
  getHeaders(): Record<string, string>;
  refresh(): void;
}

export class ApiKeyAuth implements AuthProvider {
  constructor(private apiKey: string) {}

  getHeaders(): Record<string, string> {
    return { "X-API-Key": this.apiKey };
  }

  refresh(): void {
    // No-op for static keys
  }
}

export class JwtAuth implements AuthProvider {
  private token: string | null = null;
  private refreshToken: string | null = null;
  private expiresAt: number = 0;

  constructor(
    private username: string,
    private password: string,
    private tokenUrl: string = "/v1/auth/token",
    private client?: CdfClient,
  ) {}

  getHeaders(): Record<string, string> {
    if (!this.token || Date.now() >= this.expiresAt - 60000) {
      this.refresh();
    }
    return { Authorization: `Bearer ${this.token}` };
  }

  refresh(): void {
    if (!this.client) {
      throw new Error("JWT auth requires client reference for refresh");
    }
    // Client will handle the actual refresh
  }
}

// Forward declaration to avoid circular dependency
import type { CdfClient } from "./client";
