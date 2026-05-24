package io.cdf.client;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.IOException;
import java.util.List;
import java.util.Map;

public class CdfAdmin {
    private final CdfClient client;
    private final ObjectMapper mapper = new ObjectMapper();

    public CdfAdmin(CdfClient client) {
        this.client = client;
    }

    // --- Users ---
    public JsonNode createUser(String username, String email, String password, List<String> roles) throws IOException {
        return client._request("POST", "/v1/admin/users", mapper.valueToTree(Map.of(
                "username", username, "email", email, "password", password, "roles", roles)), null);
    }

    public JsonNode listUsers() throws IOException {
        return client._request("GET", "/v1/admin/users", null, null);
    }

    public JsonNode getUser(String userId) throws IOException {
        return client._request("GET", "/v1/admin/users/" + userId, null, null);
    }

    public void deleteUser(String userId) throws IOException {
        client._request("DELETE", "/v1/admin/users/" + userId, null, null);
    }

    // --- ACL ---
    public JsonNode grantAcl(String principalId, String resourceType, String resourceId, List<String> actions)
            throws IOException {
        return client._request("POST", "/v1/admin/acl", mapper.valueToTree(Map.of(
                "principal_id", principalId, "resource_type", resourceType,
                "resource_id", resourceId, "actions", actions)), null);
    }

    public void revokeAcl(String principalId, String resourceType, String resourceId) throws IOException {
        client._request("DELETE", "/v1/admin/acl", mapper.valueToTree(Map.of(
                "principal_id", principalId, "resource_type", resourceType, "resource_id", resourceId)), null);
    }

    // --- Namespaces ---
    public JsonNode createNamespace(String name) throws IOException {
        return client._request("POST", "/v1/admin/namespaces", mapper.valueToTree(Map.of("name", name)), null);
    }

    public JsonNode listNamespaces() throws IOException {
        return client._request("GET", "/v1/admin/namespaces", null, null);
    }

    // --- Cluster ---
    public JsonNode clusterStatus() throws IOException {
        return client._request("GET", "/v1/admin/cluster/status", null, null);
    }

    public JsonNode listNodes() throws IOException {
        return client._request("GET", "/v1/admin/cluster/nodes", null, null);
    }

    // --- Backups ---
    public JsonNode createBackup(String name) throws IOException {
        return client._request("POST", "/v1/admin/backups", mapper.valueToTree(Map.of("name", name)), null);
    }

    public JsonNode listBackups() throws IOException {
        return client._request("GET", "/v1/admin/backups", null, null);
    }
}
