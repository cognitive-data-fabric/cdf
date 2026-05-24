package io.cdf.client;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.hc.client5.http.classic.methods.*;
import org.apache.hc.client5.http.impl.classic.CloseableHttpClient;
import org.apache.hc.client5.http.impl.classic.CloseableHttpResponse;
import org.apache.hc.client5.http.impl.classic.HttpClients;
import org.apache.hc.core5.http.ContentType;
import org.apache.hc.core5.http.io.entity.EntityUtils;
import org.apache.hc.core5.http.io.entity.StringEntity;

import java.io.IOException;
import java.util.List;
import java.util.Map;

public class CdfClient implements AutoCloseable {
    private final String baseUrl;
    private final AuthProvider auth;
    private final CloseableHttpClient http;
    private final ObjectMapper mapper;
    private final int retries;

    public CdfClient(String baseUrl, AuthProvider auth, int retries) {
        this.baseUrl = baseUrl.replaceAll("/$", "");
        this.auth = auth != null ? auth : new ApiKeyAuth("");
        this.http = HttpClients.createDefault();
        this.mapper = new ObjectMapper();
        this.retries = retries;
    }

    public CdfClient(String baseUrl, AuthProvider auth) {
        this(baseUrl, auth, 3);
    }

    public CdfClient(String baseUrl) {
        this(baseUrl, null, 3);
    }

    private JsonNode request(String method, String path, JsonNode body, Map<String, String> params) throws IOException {
        String url = baseUrl + path;
        if (params != null && !params.isEmpty()) {
            url += "?" + params.entrySet().stream()
                    .map(e -> e.getKey() + "=" + e.getValue())
                    .reduce((a, b) -> a + "&" + b)
                    .orElse("");
        }

        ClassicHttpRequest req;
        switch (method.toUpperCase()) {
            case "POST" -> {
                HttpPost post = new HttpPost(url);
                if (body != null)
                    post.setEntity(new StringEntity(body.toString(), ContentType.APPLICATION_JSON));
                req = post;
            }
            case "PUT" -> {
                HttpPut put = new HttpPut(url);
                if (body != null)
                    put.setEntity(new StringEntity(body.toString(), ContentType.APPLICATION_JSON));
                req = put;
            }
            case "DELETE" -> req = new HttpDelete(url);
            default -> req = new HttpGet(url);
        }

        req.setHeader("Content-Type", "application/json");
        for (Map.Entry<String, String> h : auth.getHeaders().entrySet()) {
            req.setHeader(h.getKey(), h.getValue());
        }

        IOException lastError = null;
        for (int i = 0; i < retries; i++) {
            try (CloseableHttpResponse resp = http.execute(req)) {
                String respBody = EntityUtils.toString(resp.getEntity());
                if (resp.getCode() >= 400) {
                    throw new IOException("HTTP " + resp.getCode() + ": " + respBody);
                }
                return mapper.readTree(respBody);
            } catch (IOException e) {
                lastError = e;
            }
        }
        throw lastError != null ? lastError : new IOException("Max retries exceeded");
    }

    private JsonNode request(String method, String path, JsonNode body) throws IOException {
        return request(method, path, body, null);
    }

    private JsonNode request(String method, String path) throws IOException {
        return request(method, path, null, null);
    }

    // --- Health ---
    public JsonNode health() throws IOException {
        return request("GET", "/health");
    }

    // --- CRUD ---
    public JsonNode insert(String table, Map<String, Object> data) throws IOException {
        return request("POST", "/v1/insert", mapper.valueToTree(Map.of("table", table, "data", data)));
    }

    public JsonNode batchInsert(String table, List<Map<String, Object>> rows) throws IOException {
        return request("POST", "/v1/batch_insert", mapper.valueToTree(Map.of("table", table, "rows", rows)));
    }

    public JsonNode get(String table, String rowId) throws IOException {
        return request("GET", "/v1/rows/default/" + table + "/" + rowId);
    }

    public JsonNode update(String table, String rowId, Map<String, Object> data) throws IOException {
        return request("PUT", "/v1/update", mapper.valueToTree(Map.of("table", table, "row_id", rowId, "data", data)));
    }

    public void delete(String table, String rowId) throws IOException {
        request("DELETE", "/v1/rows/default/" + table + "/" + rowId);
    }

    // --- Query ---
    public JsonNode query(String cql, Map<String, Object> params) throws IOException {
        return request("POST", "/v1/query", mapper.valueToTree(Map.of("query", cql, "params", params)));
    }

    // --- Vector Search ---
    public JsonNode search(String table, List<Double> vector, int topK, Double threshold) throws IOException {
        Map<String, Object> payload = Map.of("table", table, "vector", vector, "top_k", topK);
        if (threshold != null)
            payload.put("threshold", threshold);
        return request("POST", "/v1/search", mapper.valueToTree(payload));
    }

    public JsonNode searchText(String table, String text, int topK) throws IOException {
        return request("POST", "/v1/search_text",
                mapper.valueToTree(Map.of("table", table, "text", text, "top_k", topK)));
    }

    // --- Graph ---
    public JsonNode addEdge(String fromId, String toId, String edgeType, Map<String, Object> properties)
            throws IOException {
        Map<String, Object> payload = new java.util.HashMap<>(
                Map.of("from_id", fromId, "to_id", toId, "edge_type", edgeType));
        if (properties != null)
            payload.put("properties", properties);
        return request("POST", "/v1/graph/edges", mapper.valueToTree(payload));
    }

    public JsonNode traverse(String startId, List<String> edgeTypes, int depth) throws IOException {
        String types = edgeTypes != null ? String.join(",", edgeTypes) : "";
        return request("GET", "/v1/graph/traverse", null,
                Map.of("start_id", startId, "depth", String.valueOf(depth), "edge_types", types));
    }

    // --- Admin ---
    private CdfAdmin admin;

    public CdfAdmin admin() {
        if (admin == null)
            admin = new CdfAdmin(this);
        return admin;
    }

    // Internal
    JsonNode _request(String method, String path, JsonNode body, Map<String, String> params) throws IOException {
        return request(method, path, body, params);
    }

    @Override
    public void close() throws IOException {
        http.close();
    }
}
