package io.cdf.client;

import java.util.Map;

public class ApiKeyAuth implements AuthProvider {
    private final String apiKey;

    public ApiKeyAuth(String apiKey) {
        this.apiKey = apiKey;
    }

    @Override
    public Map<String, String> getHeaders() {
        return Map.of("X-API-Key", apiKey);
    }

    @Override
    public void refresh() {
        // No-op for static keys
    }
}
