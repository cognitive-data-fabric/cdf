export class CdfClient {
  constructor({ baseUrl, auth, timeout = 30000, retries = 3 }) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.auth = auth || { getHeaders: () => ({}), refresh: () => {} };
    this.timeout = timeout;
    this.retries = retries;
  }

  async _request(method, path, body, params) {
    const url = new URL(this.baseUrl + path);
    if (params) {
      Object.entries(params).forEach(([k, v]) => {
        if (v !== undefined && v !== null) url.searchParams.set(k, String(v));
      });
    }

    const headers = {
      "Content-Type": "application/json",
      ...this.auth.getHeaders(),
    };

    let lastError;
    for (let i = 0; i < this.retries; i++) {
      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), this.timeout);

        const resp = await fetch(url.toString(), {
          method,
          headers,
          body: body ? JSON.stringify(body) : undefined,
          signal: controller.signal,
        });
        clearTimeout(timer);

        if (resp.status === 401) {
          this.auth.refresh();
          continue;
        }

        const text = await resp.text();
        if (!resp.ok) {
          throw new Error(`HTTP ${resp.status}: ${text}`);
        }
        return text ? JSON.parse(text) : null;
      } catch (err) {
        lastError = err;
      }
    }
    throw lastError || new Error("Max retries exceeded");
  }

  // --- Health ---
  health() {
    return this._request("GET", "/health");
  }

  // --- CRUD ---
  insert(table, data, namespace = "default") {
    return this._request("POST", "/v1/insert", { namespace, table, data });
  }

  batchInsert(table, rows, namespace = "default") {
    return this._request("POST", "/v1/batch_insert", {
      namespace,
      table,
      rows,
    });
  }

  get(table, rowId, namespace = "default") {
    return this._request(
      "GET",
      `/v1/rows/${namespace}/${table}/${rowId}`,
    ).catch((err) =>
      err.message?.includes("404") ? null : Promise.reject(err),
    );
  }

  update(table, rowId, data, namespace = "default") {
    return this._request("PUT", "/v1/update", {
      namespace,
      table,
      row_id: rowId,
      data,
    });
  }

  delete(table, rowId, namespace = "default") {
    return this._request("DELETE", `/v1/rows/${namespace}/${table}/${rowId}`);
  }

  // --- Query ---
  query(cql, params) {
    return this._request("POST", "/v1/query", { query: cql, params });
  }

  // --- Vector Search ---
  search(table, vector, topK = 10, threshold, namespace = "default", filter) {
    const payload = { namespace, table, vector, top_k: topK };
    if (threshold !== undefined) payload.threshold = threshold;
    if (filter) payload.filter = filter;
    return this._request("POST", "/v1/search", payload).then(
      (r) => r.results || [],
    );
  }

  searchText(table, text, topK = 10, namespace = "default") {
    return this._request("POST", "/v1/search_text", {
      namespace,
      table,
      text,
      top_k: topK,
    }).then((r) => r.results || []);
  }

  // --- Graph ---
  addEdge(fromId, toId, edgeType, properties, namespace = "default") {
    const payload = {
      namespace,
      from_id: fromId,
      to_id: toId,
      edge_type: edgeType,
    };
    if (properties) payload.properties = properties;
    return this._request("POST", "/v1/graph/edges", payload);
  }

  traverse(startId, edgeTypes, depth = 2, namespace = "default") {
    const params = { start_id: startId, depth, namespace };
    if (edgeTypes) params.edge_types = edgeTypes.join(",");
    return this._request("GET", "/v1/graph/traverse", undefined, params).then(
      (r) => r.paths || [],
    );
  }

  neighbors(nodeId, edgeType, namespace = "default") {
    const params = { node_id: nodeId, namespace };
    if (edgeType) params.edge_type = edgeType;
    return this._request("GET", "/v1/graph/neighbors", undefined, params).then(
      (r) => r.neighbors || [],
    );
  }

  // --- Streaming ---
  subscribe(table, eventTypes, namespace = "default") {
    const url = new URL(`${this.baseUrl}/v1/subscribe`);
    url.searchParams.set("namespace", namespace);
    url.searchParams.set("table", table);
    if (eventTypes) url.searchParams.set("events", eventTypes.join(","));

    const headers = this.auth.getHeaders();
    return new EventSource(url.toString(), { headers });
  }

  // --- Admin ---
  get admin() {
    if (!this._admin) {
      this._admin = new CdfAdmin(this);
    }
    return this._admin;
  }
}

import { CdfAdmin } from "./admin.js";
