# CDF API Reference

## Base URLs

| Environment    | HTTP REST               | gRPC                     |
| -------------- | ----------------------- | ------------------------ |
| Local          | `http://localhost:8080` | `grpc://localhost:50053` |
| Docker Compose | `http://localhost:8080` | `grpc://localhost:50053` |

---

## REST Endpoints

### Health

```http
GET /health
```

**Response:**

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "components": {
    "storage": "healthy",
    "router": "healthy",
    "meta": "healthy"
  }
}
```

---

### Query

```http
POST /v1/query
Content-Type: application/json
```

**Request:**

```json
{
  "request_id": "req-123",
  "cql": "SELECT title FROM papers WHERE embedding SIMILAR TO [0.1, 0.2, ...] WITH THRESHOLD 0.85 LIMIT 10",
  "params": {
    ":query_embedding": [0.1, 0.2, 0.3, 0.4]
  },
  "consistency": "eventual",
  "timeout_ms": 5000
}
```

**Response:**

```json
{
  "request_id": "req-123",
  "status": "ok",
  "results": [
    {
      "title": "Attention Is All You Need",
      "confidence": 0.95,
      "_score": 0.92,
      "_distance": 0.08
    }
  ],
  "meta": {
    "total_hits": 150,
    "shards_queried": 3,
    "query_time_ms": 45
  }
}
```

---

### Insert

```http
POST /v1/insert
Content-Type: application/json
```

**Request:**

```json
{
  "request_id": "req-124",
  "table": "papers",
  "data": {
    "id": "paper-001",
    "title": "Attention Is All You Need",
    "abstract": "We propose a new simple network architecture...",
    "embedding": {
      "model_id": "all-MiniLM-L6-v2",
      "values": [0.1, 0.2, 0.3, 0.4, 0.5]
    },
    "published_year": 2017,
    "confidence": {
      "type": "normal",
      "params": { "mean": 0.95, "std_dev": 0.05 }
    }
  },
  "generate_embedding": false
}
```

**Response:**

```json
{
  "request_id": "req-124",
  "status": "ok",
  "row_id": "paper-001",
  "sequence_number": 42,
  "shard": 3
}
```

---

### Vector Search

```http
POST /v1/search
Content-Type: application/json
```

**Request:**

```json
{
  "request_id": "req-125",
  "table": "papers",
  "vector": [0.1, 0.2, 0.3, 0.4],
  "vector_column": "embedding",
  "top_k": 10,
  "threshold": 0.85,
  "metric": "cosine",
  "include_metadata": true
}
```

**Response:**

```json
{
  "request_id": "req-125",
  "status": "ok",
  "results": [
    {
      "id": "paper-001",
      "similarity": 0.92,
      "distance": 0.08,
      "data": {
        "title": "Attention Is All You Need",
        "abstract": "..."
      }
    }
  ],
  "meta": {
    "total_candidates": 10000,
    "index_time_ms": 5,
    "rerank_time_ms": 2
  }
}
```

---

### Graph Traversal

```http
POST /v1/traverse
Content-Type: application/json
```

**Request:**

```json
{
  "request_id": "req-126",
  "start_node": "paper-001",
  "edge_types": ["cites", "references"],
  "max_depth": 3,
  "direction": "outgoing",
  "include_properties": true
}
```

**Response:**

```json
{
  "request_id": "req-126",
  "status": "ok",
  "paths": [
    {
      "nodes": ["paper-001", "paper-002", "paper-003"],
      "edges": [
        { "from": "paper-001", "to": "paper-002", "type": "cites" },
        { "from": "paper-002", "to": "paper-003", "type": "references" }
      ],
      "length": 2
    }
  ]
}
```

---

### Batch Insert

```http
POST /v1/batch
Content-Type: application/json
```

**Request:**

```json
{
  "request_id": "req-127",
  "table": "papers",
  "rows": [
    { "id": "paper-001", "title": "Paper One", "embedding": {...} },
    { "id": "paper-002", "title": "Paper Two", "embedding": {...} }
  ],
  "atomic": true
}
```

**Response:**

```json
{
  "request_id": "req-127",
  "status": "ok",
  "inserted": 2,
  "failed": 0,
  "sequence_numbers": [43, 44]
}
```

---

### Schema Operations

```http
POST /v1/schema/create
Content-Type: application/json
```

**Request:**

```json
{
  "name": "papers",
  "columns": [
    { "name": "id", "type": "string", "nullable": false },
    { "name": "title", "type": "string", "nullable": false },
    {
      "name": "embedding",
      "type": "vector",
      "dimensions": 384,
      "metric": "cosine"
    },
    { "name": "published_date", "type": "temporal" }
  ],
  "indexes": [
    { "name": "idx_embedding", "type": "hnsw", "columns": ["embedding"] },
    {
      "name": "idx_temporal",
      "type": "temporal",
      "columns": ["published_date"]
    }
  ]
}
```

---

### Streaming Query (WebSocket)

```http
GET /v1/stream?query=SELECT * FROM events WHERE embedding SIMILAR TO :q
Upgrade: websocket
```

**Messages:**

```json
// Server → Client (results)
{ "type": "result", "data": {...} }
{ "type": "result", "data": {...} }

// Server → Client (complete)
{ "type": "complete", "meta": {"total": 100} }
```

---

## Embedding Service Endpoints

### Generate Embeddings

```http
POST http://localhost:8001/embed
Content-Type: application/json
```

**Request:**

```json
{
  "texts": ["The quick brown fox", "Vector databases are essential for AI"],
  "model_id": "all-MiniLM-L6-v2",
  "normalize": true,
  "batch_size": 32
}
```

**Response:**

```json
{
  "embeddings": [
    [0.023, -0.156, 0.341, ...],
    [0.111, 0.222, -0.333, ...]
  ],
  "model_id": "all-MiniLM-L6-v2",
  "dimensions": 384,
  "processing_time_ms": 12.5
}
```

### Single Embedding

```http
POST http://localhost:8001/embed/single?text=Hello+world&model_id=all-MiniLM-L6-v2
```

**Response:**

```json
{
  "embedding": [0.023, -0.156, 0.341, ...],
  "model_id": "all-MiniLM-L6-v2"
}
```

### List Models

```http
GET http://localhost:8001/models
```

**Response:**

```json
{
  "loaded": ["all-MiniLM-L6-v2"],
  "available": ["all-MiniLM-L6-v2", "all-MiniLM-L12-v2", "all-mpnet-base-v2"]
}
```

---

## Drift Detector Endpoints

### Register Reference Distribution

```http
POST http://localhost:8002/register-reference
Content-Type: application/json
```

**Request:**

```json
{
  "concept_id": "sports",
  "embeddings": [
    [0.1, 0.2, ...],
    [0.3, 0.4, ...]
  ]
}
```

### Check for Drift

```http
POST http://localhost:8002/check-drift?threshold=0.05
Content-Type: application/json
```

**Request:**

```json
{
  "concept_id": "sports",
  "embeddings": [
    [0.15, 0.25, ...],
    [0.35, 0.45, ...]
  ]
}
```

**Response:**

```json
{
  "concept_id": "sports",
  "drift_detected": true,
  "drift_score": 0.82,
  "reference_mean": [0.1, 0.2, ...],
  "current_mean": [0.15, 0.25, ...],
  "p_value": 0.001,
  "method": "mmd+wasserstein"
}
```

---

## Error Responses

All errors follow this format:

```json
{
  "request_id": "req-123",
  "status": "error",
  "error": {
    "code": "VECTOR_DIMENSION_MISMATCH",
    "message": "Vector dimension mismatch: expected 384, got 512",
    "component": "cdf-storage",
    "retryable": false
  }
}
```

**Error Codes:**

| Code                        | HTTP Status | Description                 |
| --------------------------- | ----------- | --------------------------- |
| `QUERY_PARSE`               | 400         | Invalid CQL syntax          |
| `SCHEMA_VALIDATION`         | 400         | Schema constraint violation |
| `VECTOR_DIMENSION_MISMATCH` | 400         | Wrong embedding dimensions  |
| `STORAGE_NOT_FOUND`         | 404         | Key does not exist          |
| `SHARD_NOT_FOUND`           | 404         | Invalid shard ID            |
| `QUORUM_UNAVAILABLE`        | 503         | Not enough nodes available  |
| `QUERY_TIMEOUT`             | 504         | Query exceeded timeout      |

---

## Rate Limiting

Headers included in all responses:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1625097600
```

---

## Authentication

CDF supports multiple auth methods:

### API Key (Header)

```http
Authorization: Bearer cdf_sk_abc123xyz
```

### mTLS (Service-to-Service)

```bash
curl --cert client.crt --key client.key \
     --cacert ca.crt \
     https://localhost:8080/v1/query
```

---

## Pagination

For large result sets:

```http
POST /v1/query
Content-Type: application/json

{
  "cql": "SELECT * FROM papers LIMIT 100",
  "cursor": "eyJwYWdlIjogMn0="
}
```

**Response includes:**

```json
{
  "results": [...],
  "has_more": true,
  "next_cursor": "eyJwYWdlIjogM30="
}
```

---

## Authentication Endpoints

### Login (JWT)

```http
POST /v1/auth/token
Content-Type: application/json
```

**Request:**

```json
{
  "username": "alice",
  "password": "secret123",
  "mfa_code": "123456"
}
```

**Response:**

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIs...",
  "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
  "token_type": "Bearer",
  "expires_in": 3600
}
```

### Refresh Token

```http
POST /v1/auth/refresh
Content-Type: application/json
```

**Request:**

```json
{
  "refresh_token": "eyJhbGciOiJIUzI1NiIs..."
}
```

### Revoke Token

```http
POST /v1/auth/revoke
Content-Type: application/json
Authorization: Bearer <token>
```

**Request:**

```json
{
  "token_id": "token-uuid-here"
}
```

---

## Admin Endpoints

### User Management

#### Create User

```http
POST /v1/admin/users
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "username": "alice",
  "email": "alice@example.com",
  "password": "secret123",
  "roles": ["developer"],
  "namespaces": ["default", "production"]
}
```

#### List Users

```http
GET /v1/admin/users?namespace=default&role=developer
Authorization: Bearer <admin-token>
```

**Response:**

```json
{
  "users": [
    {
      "id": "user-001",
      "username": "alice",
      "email": "alice@example.com",
      "roles": ["developer"],
      "namespaces": ["default"],
      "created_at": "2024-01-15T10:00:00Z",
      "disabled": false,
      "mfa_enabled": false
    }
  ]
}
```

#### Update User

```http
PUT /v1/admin/users/{user_id}
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "roles": ["admin"],
  "namespaces": ["default", "production"],
  "disabled": false,
  "mfa_enabled": true
}
```

#### Delete User

```http
DELETE /v1/admin/users/{user_id}
Authorization: Bearer <admin-token>
```

#### Reset Password

```http
POST /v1/admin/users/{user_id}/reset_password
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "password": "new-secret-456"
}
```

---

### API Key Management

#### Create API Key

```http
POST /v1/admin/apikeys
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "name": "prod-etl-service",
  "user_id": "user-001",
  "roles": ["service"],
  "expires_in_days": 365
}
```

**Response:**

```json
{
  "key_id": "key-001",
  "api_key": "cdf-key-1234567890abcdef",
  "name": "prod-etl-service",
  "created_at": "2024-01-15T10:00:00Z",
  "expires_at": "2025-01-15T10:00:00Z"
}
```

#### List API Keys

```http
GET /v1/admin/apikeys?user_id=user-001
Authorization: Bearer <admin-token>
```

#### Revoke API Key

```http
DELETE /v1/admin/apikeys/{key_id}
Authorization: Bearer <admin-token>
```

---

### ACL Management

#### Grant ACL

```http
POST /v1/admin/acl
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "principal_id": "user-001",
  "resource_type": "table",
  "resource_id": "documents",
  "actions": ["read", "write"],
  "namespace": "default",
  "expires_at": "2025-01-01T00:00:00Z",
  "conditions": {
    "ip_range": "10.0.0.0/8",
    "time_window": "09:00-18:00"
  }
}
```

#### Revoke ACL

```http
DELETE /v1/admin/acl
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "principal_id": "user-001",
  "resource_type": "table",
  "resource_id": "documents",
  "namespace": "default",
  "actions": ["write"]
}
```

#### List ACL

```http
GET /v1/admin/acl?principal_id=user-001&namespace=default
Authorization: Bearer <admin-token>
```

#### Get Permissions

```http
GET /v1/admin/permissions/{principal_id}
Authorization: Bearer <admin-token>
```

**Response:**

```json
{
  "principal_id": "user-001",
  "table_perms": {
    "documents": ["read", "write", "create"],
    "users": ["read"]
  },
  "vector_perms": {
    "idx_embeddings": ["search"]
  },
  "system_admin": false
}
```

---

### Namespace Management

#### Create Namespace

```http
POST /v1/admin/namespaces
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "name": "production",
  "owner": "user-001",
  "quotas": {
    "max_tables": 100,
    "max_storage_gb": 500
  }
}
```

#### List Namespaces

```http
GET /v1/admin/namespaces
Authorization: Bearer <admin-token>
```

#### Get Namespace

```http
GET /v1/admin/namespaces/{name}
Authorization: Bearer <admin-token>
```

#### Update Namespace

```http
PUT /v1/admin/namespaces/{name}
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "owner": "user-002",
  "quotas": {
    "max_tables": 200,
    "max_storage_gb": 1000
  }
}
```

#### Delete Namespace

```http
DELETE /v1/admin/namespaces/{name}?force=false
Authorization: Bearer <admin-token>
```

---

### Cluster Management

#### Cluster Status

```http
GET /v1/admin/cluster/status
Authorization: Bearer <admin-token>
```

**Response:**

```json
{
  "version": "0.1.0",
  "nodes": [
    {
      "node_id": "node-001",
      "role": "storage",
      "status": "healthy",
      "shard_range": [0, 1000],
      "cpu_percent": 45,
      "memory_percent": 60
    }
  ],
  "shards": {
    "total": 32,
    "active": 32,
    "rebalancing": false
  }
}
```

#### List Nodes

```http
GET /v1/admin/cluster/nodes
Authorization: Bearer <admin-token>
```

#### Drain Node

```http
POST /v1/admin/cluster/nodes/{node_id}/drain
Authorization: Bearer <admin-token>
```

#### Activate Node

```http
POST /v1/admin/cluster/nodes/{node_id}/activate
Authorization: Bearer <admin-token>
```

#### Rebalance Shards

```http
POST /v1/admin/cluster/rebalance
Authorization: Bearer <admin-token>
```

---

### Backup Management

#### Create Backup

```http
POST /v1/admin/backups
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "name": "daily-2024-01-15",
  "tables": ["documents", "users"]
}
```

#### List Backups

```http
GET /v1/admin/backups
Authorization: Bearer <admin-token>
```

#### Restore Backup

```http
POST /v1/admin/backups/restore
Content-Type: application/json
Authorization: Bearer <admin-token>
```

**Request:**

```json
{
  "backup_id": "backup-001",
  "target_namespace": "restored"
}
```

#### Delete Backup

```http
DELETE /v1/admin/backups/{backup_id}
Authorization: Bearer <admin-token>
```

---

### Audit Logs

#### List Audit Logs

```http
GET /v1/admin/audit?principal_id=user-001&action=insert&limit=100
Authorization: Bearer <admin-token>
```

**Response:**

```json
{
  "entries": [
    {
      "id": "audit-001",
      "timestamp": "2024-01-15T10:30:00Z",
      "principal_id": "user-001",
      "action": "insert",
      "resource": "table:default:documents",
      "allowed": true,
      "ip_address": "10.0.1.5",
      "user_agent": "cdf-client/0.1.0"
    }
  ]
}
```

---

## Authentication & Authorization Summary

### Required Roles by Endpoint

| Endpoint                    | Required Role |
| --------------------------- | ------------- |
| `GET /health`               | None          |
| `POST /v1/query`            | Analyst+      |
| `POST /v1/insert`           | Developer+    |
| `POST /v1/search`           | Analyst+      |
| `POST /v1/traverse`         | Analyst+      |
| `POST /v1/admin/*`          | Admin+        |
| `POST /v1/admin/cluster/*`  | SuperAdmin    |
| `POST /v1/admin/namespaces` | SuperAdmin    |

### HTTP Status Codes

| Status | Meaning                              |
| ------ | ------------------------------------ |
| 200    | Success                              |
| 201    | Created (user, key, namespace)       |
| 204    | No Content (delete)                  |
| 400    | Bad Request                          |
| 401    | Unauthorized (invalid/missing token) |
| 403    | Forbidden (insufficient permissions) |
| 404    | Not Found                            |
| 422    | Validation Error                     |
| 429    | Rate Limited                         |
| 500    | Internal Server Error                |
| 503    | Service Unavailable                  |
