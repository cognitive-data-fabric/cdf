import { CdfClient } from "./client";

export class CdfAdmin {
  constructor(private client: CdfClient) {}

  // --- Authentication ---
  login(username: string, password: string): Promise<Record<string, unknown>> {
    return this.client._request("POST", "/v1/auth/token", {
      username,
      password,
    });
  }

  refreshToken(refreshToken: string): Promise<Record<string, unknown>> {
    return this.client._request("POST", "/v1/auth/refresh", {
      refresh_token: refreshToken,
    });
  }

  revokeToken(tokenId: string): Promise<void> {
    return this.client._request("POST", "/v1/auth/revoke", {
      token_id: tokenId,
    });
  }

  // --- User Management ---
  createUser(
    username: string,
    email: string,
    password: string,
    roles: string[],
    namespaces?: string[],
  ): Promise<Record<string, unknown>> {
    return this.client._request("POST", "/v1/admin/users", {
      username,
      email,
      password,
      roles,
      namespaces: namespaces || ["default"],
    });
  }

  listUsers(
    namespace?: string,
    role?: string,
  ): Promise<Record<string, unknown>[]> {
    const params: Record<string, unknown> = {};
    if (namespace) params.namespace = namespace;
    if (role) params.role = role;
    return this.client
      ._request("GET", "/v1/admin/users", undefined, params)
      .then((r: any) => r.users || []);
  }

  getUser(userId: string): Promise<Record<string, unknown>> {
    return this.client._request("GET", `/v1/admin/users/${userId}`);
  }

  updateUser(
    userId: string,
    updates: {
      roles?: string[];
      namespaces?: string[];
      disabled?: boolean;
      mfaEnabled?: boolean;
    },
  ): Promise<Record<string, unknown>> {
    const payload: Record<string, unknown> = {};
    if (updates.roles !== undefined) payload.roles = updates.roles;
    if (updates.namespaces !== undefined)
      payload.namespaces = updates.namespaces;
    if (updates.disabled !== undefined) payload.disabled = updates.disabled;
    if (updates.mfaEnabled !== undefined)
      payload.mfa_enabled = updates.mfaEnabled;
    return this.client._request("PUT", `/v1/admin/users/${userId}`, payload);
  }

  deleteUser(userId: string): Promise<void> {
    return this.client._request("DELETE", `/v1/admin/users/${userId}`);
  }

  resetPassword(userId: string, newPassword: string): Promise<void> {
    return this.client._request(
      "POST",
      `/v1/admin/users/${userId}/reset_password`,
      {
        password: newPassword,
      },
    );
  }

  // --- API Keys ---
  createApiKey(
    name: string,
    userId: string,
    roles: string[],
    expiresInDays = 365,
  ): Promise<Record<string, unknown>> {
    return this.client._request("POST", "/v1/admin/apikeys", {
      name,
      user_id: userId,
      roles,
      expires_in_days: expiresInDays,
    });
  }

  listApiKeys(userId?: string): Promise<Record<string, unknown>[]> {
    const params: Record<string, unknown> = {};
    if (userId) params.user_id = userId;
    return this.client
      ._request("GET", "/v1/admin/apikeys", undefined, params)
      .then((r: any) => r.keys || []);
  }

  revokeApiKey(keyId: string): Promise<void> {
    return this.client._request("DELETE", `/v1/admin/apikeys/${keyId}`);
  }

  // --- ACL Management ---
  grantAcl(params: {
    principalId: string;
    resourceType: string;
    resourceId: string;
    actions: string[];
    namespace?: string;
    expiresAt?: string;
    conditions?: Record<string, unknown>;
  }): Promise<Record<string, unknown>> {
    const payload: Record<string, unknown> = {
      principal_id: params.principalId,
      resource_type: params.resourceType,
      resource_id: params.resourceId,
      actions: params.actions,
      namespace: params.namespace || "default",
    };
    if (params.expiresAt) payload.expires_at = params.expiresAt;
    if (params.conditions) payload.conditions = params.conditions;
    return this.client._request("POST", "/v1/admin/acl", payload);
  }

  revokeAcl(params: {
    principalId: string;
    resourceType: string;
    resourceId: string;
    actions?: string[];
    namespace?: string;
  }): Promise<void> {
    const payload: Record<string, unknown> = {
      principal_id: params.principalId,
      resource_type: params.resourceType,
      resource_id: params.resourceId,
      namespace: params.namespace || "default",
    };
    if (params.actions) payload.actions = params.actions;
    return this.client._request("DELETE", "/v1/admin/acl", payload);
  }

  listAcl(params?: {
    principalId?: string;
    resourceType?: string;
    namespace?: string;
  }): Promise<Record<string, unknown>[]> {
    const query: Record<string, unknown> = {};
    if (params?.principalId) query.principal_id = params.principalId;
    if (params?.resourceType) query.resource_type = params.resourceType;
    if (params?.namespace) query.namespace = params.namespace;
    return this.client
      ._request("GET", "/v1/admin/acl", undefined, query)
      .then((r: any) => r.entries || []);
  }

  getPermissions(principalId: string): Promise<Record<string, unknown>> {
    return this.client._request("GET", `/v1/admin/permissions/${principalId}`);
  }

  // --- Namespace Management ---
  createNamespace(
    name: string,
    owner?: string,
  ): Promise<Record<string, unknown>> {
    const payload: Record<string, unknown> = { name };
    if (owner) payload.owner = owner;
    return this.client._request("POST", "/v1/admin/namespaces", payload);
  }

  listNamespaces(): Promise<Record<string, unknown>[]> {
    return this.client
      ._request("GET", "/v1/admin/namespaces")
      .then((r: any) => r.namespaces || []);
  }

  getNamespace(name: string): Promise<Record<string, unknown>> {
    return this.client._request("GET", `/v1/admin/namespaces/${name}`);
  }

  updateNamespace(
    name: string,
    updates: { owner?: string; quotas?: Record<string, unknown> },
  ): Promise<Record<string, unknown>> {
    const payload: Record<string, unknown> = {};
    if (updates.owner) payload.owner = updates.owner;
    if (updates.quotas) payload.quotas = updates.quotas;
    return this.client._request("PUT", `/v1/admin/namespaces/${name}`, payload);
  }

  deleteNamespace(name: string, force = false): Promise<void> {
    return this.client._request(
      "DELETE",
      `/v1/admin/namespaces/${name}`,
      undefined,
      {
        force: force ? "true" : "false",
      },
    );
  }

  // --- Cluster Management ---
  clusterStatus(): Promise<Record<string, unknown>> {
    return this.client._request("GET", "/v1/admin/cluster/status");
  }

  listNodes(): Promise<Record<string, unknown>[]> {
    return this.client
      ._request("GET", "/v1/admin/cluster/nodes")
      .then((r: any) => r.nodes || []);
  }

  drainNode(nodeId: string): Promise<void> {
    return this.client._request(
      "POST",
      `/v1/admin/cluster/nodes/${nodeId}/drain`,
    );
  }

  activateNode(nodeId: string): Promise<void> {
    return this.client._request(
      "POST",
      `/v1/admin/cluster/nodes/${nodeId}/activate`,
    );
  }

  rebalanceShards(): Promise<Record<string, unknown>> {
    return this.client._request("POST", "/v1/admin/cluster/rebalance");
  }

  // --- Backups ---
  createBackup(
    name: string,
    tables?: string[],
  ): Promise<Record<string, unknown>> {
    const payload: Record<string, unknown> = { name };
    if (tables) payload.tables = tables;
    return this.client._request("POST", "/v1/admin/backups", payload);
  }

  listBackups(): Promise<Record<string, unknown>[]> {
    return this.client
      ._request("GET", "/v1/admin/backups")
      .then((r: any) => r.backups || []);
  }

  restoreBackup(
    backupId: string,
    targetNamespace?: string,
  ): Promise<Record<string, unknown>> {
    const payload: Record<string, unknown> = { backup_id: backupId };
    if (targetNamespace) payload.target_namespace = targetNamespace;
    return this.client._request("POST", "/v1/admin/backups/restore", payload);
  }

  deleteBackup(backupId: string): Promise<void> {
    return this.client._request("DELETE", `/v1/admin/backups/${backupId}`);
  }

  // --- Audit Logs ---
  listAuditLogs(params?: {
    principalId?: string;
    action?: string;
    resource?: string;
    startTime?: string;
    endTime?: string;
    limit?: number;
  }): Promise<Record<string, unknown>[]> {
    const query: Record<string, unknown> = { limit: params?.limit || 100 };
    if (params?.principalId) query.principal_id = params.principalId;
    if (params?.action) query.action = params.action;
    if (params?.resource) query.resource = params.resource;
    if (params?.startTime) query.start_time = params.startTime;
    if (params?.endTime) query.end_time = params.endTime;
    return this.client
      ._request("GET", "/v1/admin/audit", undefined, query)
      .then((r: any) => r.entries || []);
  }

  // --- Schema Registry ---
  registerSchema(
    name: string,
    schema: Record<string, unknown>,
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    return this.client._request("POST", "/v1/admin/schemas", {
      name,
      namespace,
      schema,
    });
  }

  getSchema(
    name: string,
    namespace = "default",
    version?: number,
  ): Promise<Record<string, unknown>> {
    const params: Record<string, unknown> = {};
    if (version) params.version = version;
    return this.client._request(
      "GET",
      `/v1/admin/schemas/${namespace}/${name}`,
      undefined,
      params,
    );
  }

  listSchemas(namespace = "default"): Promise<Record<string, unknown>[]> {
    return this.client
      ._request("GET", "/v1/admin/schemas", undefined, { namespace })
      .then((r: any) => r.schemas || []);
  }

  evolveSchema(
    name: string,
    changes: Record<string, unknown>[],
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    return this.client._request("POST", "/v1/admin/schemas/evolve", {
      namespace,
      name,
      changes,
    });
  }
}
