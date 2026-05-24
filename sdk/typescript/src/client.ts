import axios, { AxiosInstance, AxiosResponse } from "axios";
import { EventSource } from "eventsource";
import { AuthProvider, ApiKeyAuth } from "./auth";
import { CdfAdmin } from "./admin";

export interface CdfConfig {
  baseUrl: string;
  auth?: AuthProvider;
  timeout?: number;
  retries?: number;
}

export class CdfClient {
  private http: AxiosInstance;
  private auth: AuthProvider;
  private retries: number;

  constructor(config: CdfConfig) {
    const baseUrl = config.baseUrl.replace(/\/$/, "");
    this.auth = config.auth || new ApiKeyAuth("");
    this.retries = config.retries || 3;

    this.http = axios.create({
      baseURL: baseUrl,
      timeout: config.timeout || 30000,
      headers: {
        "Content-Type": "application/json",
      },
    });

    this.http.interceptors.request.use((config) => {
      Object.entries(this.auth.getHeaders()).forEach(([key, value]) => {
        config.headers.set(key, value);
      });
      return config;
    });
  }

  private async request<T>(
    method: string,
    path: string,
    data?: unknown,
    params?: Record<string, unknown>,
  ): Promise<T> {
    let lastError: Error | null = null;
    for (let attempt = 0; attempt < this.retries; attempt++) {
      try {
        const resp: AxiosResponse<T> = await this.http.request({
          method,
          url: path,
          data,
          params,
        });
        return resp.data;
      } catch (err: any) {
        lastError = err;
        if (err.response?.status === 401) {
          this.auth.refresh();
        }
      }
    }
    throw lastError || new Error("Max retries exceeded");
  }

  // --- Health ---
  health(): Promise<Record<string, unknown>> {
    return this.request("GET", "/health");
  }

  // --- Table Operations ---
  createTable(
    name: string,
    schema?: Record<string, unknown>,
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    return this.request("POST", "/v1/tables", { name, namespace, schema });
  }

  listTables(namespace = "default"): Promise<Record<string, unknown>[]> {
    return this.request("GET", "/v1/tables", undefined, { namespace }).then(
      (r: any) => r.tables || [],
    );
  }

  getTable(
    name: string,
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    return this.request("GET", `/v1/tables/${namespace}/${name}`);
  }

  dropTable(name: string, namespace = "default"): Promise<void> {
    return this.request("DELETE", `/v1/tables/${namespace}/${name}`);
  }

  // --- CRUD ---
  insert(
    table: string,
    data: Record<string, unknown>,
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    return this.request("POST", "/v1/insert", { namespace, table, data });
  }

  batchInsert(
    table: string,
    rows: Record<string, unknown>[],
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    return this.request("POST", "/v1/batch_insert", { namespace, table, rows });
  }

  get(
    table: string,
    rowId: string,
    namespace = "default",
  ): Promise<Record<string, unknown> | null> {
    return this.request<Record<string, unknown>>(
      "GET",
      `/v1/rows/${namespace}/${table}/${rowId}`,
    ).catch((err: any) =>
      err.response?.status === 404 ? null : Promise.reject(err),
    ) as Promise<Record<string, unknown> | null>;
  }

  update(
    table: string,
    rowId: string,
    data: Record<string, unknown>,
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    return this.request("PUT", "/v1/update", {
      namespace,
      table,
      row_id: rowId,
      data,
    });
  }

  delete(table: string, rowId: string, namespace = "default"): Promise<void> {
    return this.request("DELETE", `/v1/rows/${namespace}/${table}/${rowId}`);
  }

  // --- Query ---
  query(
    cql: string,
    params?: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return this.request("POST", "/v1/query", { query: cql, params });
  }

  // --- Vector Search ---
  search(
    table: string,
    vector: number[],
    topK = 10,
    threshold?: number,
    namespace = "default",
    filter?: Record<string, unknown>,
  ): Promise<Record<string, unknown>[]> {
    const payload: Record<string, unknown> = {
      namespace,
      table,
      vector,
      top_k: topK,
    };
    if (threshold !== undefined) payload.threshold = threshold;
    if (filter) payload.filter = filter;
    return this.request("POST", "/v1/search", payload).then(
      (r: any) => r.results || [],
    );
  }

  searchText(
    table: string,
    text: string,
    topK = 10,
    namespace = "default",
  ): Promise<Record<string, unknown>[]> {
    return this.request("POST", "/v1/search_text", {
      namespace,
      table,
      text,
      top_k: topK,
    }).then((r: any) => r.results || []);
  }

  // --- Graph ---
  addEdge(
    fromId: string,
    toId: string,
    edgeType: string,
    properties?: Record<string, unknown>,
    namespace = "default",
  ): Promise<Record<string, unknown>> {
    const payload: Record<string, unknown> = {
      namespace,
      from_id: fromId,
      to_id: toId,
      edge_type: edgeType,
    };
    if (properties) payload.properties = properties;
    return this.request("POST", "/v1/graph/edges", payload);
  }

  traverse(
    startId: string,
    edgeTypes?: string[],
    depth = 2,
    namespace = "default",
  ): Promise<Record<string, unknown>[]> {
    const params: Record<string, unknown> = {
      start_id: startId,
      depth,
      namespace,
    };
    if (edgeTypes) params.edge_types = edgeTypes.join(",");
    return this.request("GET", "/v1/graph/traverse", undefined, params).then(
      (r: any) => r.paths || [],
    );
  }

  neighbors(
    nodeId: string,
    edgeType?: string,
    namespace = "default",
  ): Promise<Record<string, unknown>[]> {
    const params: Record<string, unknown> = { node_id: nodeId, namespace };
    if (edgeType) params.edge_type = edgeType;
    return this.request("GET", "/v1/graph/neighbors", undefined, params).then(
      (r: any) => r.neighbors || [],
    );
  }

  // --- Streaming ---
  subscribe(table: string, eventTypes?: string[], namespace = "default"): any {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { EventSource } = require("eventsource");
    const url = new URL(`${this.http.defaults.baseURL}/v1/subscribe`);
    url.searchParams.set("namespace", namespace);
    url.searchParams.set("table", table);
    if (eventTypes) url.searchParams.set("events", eventTypes.join(","));

    const headers = this.auth.getHeaders();
    return new EventSource(url.toString(), { headers });
  }

  // --- Admin ---
  private _admin?: CdfAdmin;

  get admin(): CdfAdmin {
    if (!this._admin) this._admin = new CdfAdmin(this);
    return this._admin;
  }

  // Internal method for admin
  _request<T>(
    method: string,
    path: string,
    data?: unknown,
    params?: Record<string, unknown>,
  ): Promise<T> {
    return this.request(method, path, data, params);
  }
}
