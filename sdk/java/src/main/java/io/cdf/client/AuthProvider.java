package io.cdf.client;

import java.util.Map;

public interface AuthProvider {
    Map<String, String> getHeaders();

    void refresh();
}
