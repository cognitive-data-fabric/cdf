export class CdfAdmin {
  constructor(client) {
    this.client = client;
  }

  // --- Users ---
  createUser(username, email, password, roles) {
    return this.client._request("POST", "/v1/admin/users", {
      username,
      email,
      password,
      roles,
    });
  }

  listUsers(namespace, role) {
    const params = {};
    if (namespace) params.namespace = namespace;
    if (role) params.role = role;
    return this.client
      ._request("GET", "/v1/admin/users", undefined, params)
      .then((r) => r.users || []);
  }

  getUser(userId) {
    return this.client._request("GET", `/v1/admin/users/${userId}`);
  }

  updateUser(userId, updates) {
    const payload = {};
    if (updates.roles !== undefined) payload.roles = updates.roles;
    if (updates.namespaces !== undefined)
      payload.namespaces = updates.namespaces;
    if (updates.disabled !== undefined) payload.disabled = updates.disabled;
    if (updates.mfaEnabled !== undefined)
      payload.mfa_enabled = updates.mfaEnabled;
    return this.client._request("PUT", `/v1/admin/users/${userId}`, payload);
  }

  deleteUser(userId) {
    return this.client._request("DELETE", `/v1/admin/users/${userId}`);
  }

  // --- API Keys ---
  createApiKey(name, userId, roles, expiresInDays = 365) {
    return this.client._request("POST", "/v1/admin/apikeys", {
      name,
      user_id: userId,
      roles,
      expires_in_days: expiresInDays,
    });
  }

  listApiKeys(userId) {
    const params = userId ? { user_id: userId } : {};
    return this.client
      ._request("GET", "/v1/admin/apikeys", undefined, params)
      .then((r) => r.keys || []);
  }

  revokeApiKey(keyId) {
    return this.client._request("DELETE", `/v1/admin/apikeys/${keyId}`);
  }

  // --- ACL ---
  grantAcl({
    principalId,
    resourceType,
    resourceId,
    actions,
    namespace = "default",
    expiresAt,
    conditions,
  }) {
    const payload = {
      principal_id: principalId,
      resource_type: resourceType,
      resource_id: resourceId,
      actions,
      namespace,
    };
    if (expiresAt) payload.expires_at = expiresAt;
    if (conditions) payload.conditions = conditions;
    return this.client._request("POST", "/v1/admin/acl", payload);
  }

  revokeAcl({
    principalId,
    resourceType,
    resourceId,
    actions,
    namespace = "default",
  }) {
    const payload = {
      principal_id: principalId,
      resource_type: resourceType,
      resource_id: resourceId,
      namespace,
    };
    if (actions) payload.actions = actions;
    return this.client._request("DELETE", "/v1/admin/acl", payload);
  }

  listAcl({ principalId, resourceType, namespace } = {}) {
    const params = {};
    if (principalId) params.principal_id = principalId;
    if (resourceType) params.resource_type = resourceType;
    if (namespace) params.namespace = namespace;
    return this.client
      ._request("GET", "/v1/admin/acl", undefined, params)
      .then((r) => r.entries || []);
  }

  // --- Namespaces ---
  createNamespace(name, owner) {
    const payload = { name };
    if (owner) payload.owner = owner;
    return this.client._request("POST", "/v1/admin/namespaces", payload);
  }

  listNamespaces() {
    return this.client
      ._request("GET", "/v1/admin/namespaces")
      .then((r) => r.namespaces || []);
  }

  // --- Cluster ---
  clusterStatus() {
    return this.client._request("GET", "/v1/admin/cluster/status");
  }

  listNodes() {
    return this.client
      ._request("GET", "/v1/admin/cluster/nodes")
      .then((r) => r.nodes || []);
  }

  drainNode(nodeId) {
    return this.client._request(
      "POST",
      `/v1/admin/cluster/nodes/${nodeId}/drain`,
    );
  }

  activateNode(nodeId) {
    return this.client._request(
      "POST",
      `/v1/admin/cluster/nodes/${nodeId}/activate`,
    );
  }

  rebalanceShards() {
    return this.client._request("POST", "/v1/admin/cluster/rebalance");
  }

  // --- Backups ---
  createBackup(name, tables) {
    const payload = { name };
    if (tables) payload.tables = tables;
    return this.client._request("POST", "/v1/admin/backups", payload);
  }

  listBackups() {
    return this.client
      ._request("GET", "/v1/admin/backups")
      .then((r) => r.backups || []);
  }

  restoreBackup(backupId, targetNamespace) {
    const payload = { backup_id: backupId };
    if (targetNamespace) payload.target_namespace = targetNamespace;
    return this.client._request("POST", "/v1/admin/backups/restore", payload);
  }

  deleteBackup(backupId) {
    return this.client._request("DELETE", `/v1/admin/backups/${backupId}`);
  }
}
