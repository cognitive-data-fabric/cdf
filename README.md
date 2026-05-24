# Cognitive Data Fabric (CDF)

> A new kind of database for the AI era — unified semantic, graph, and temporal storage with native uncertainty handling.

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://rust-lang.org)
[![Go](https://img.shields.io/badge/Go-1.22%2B-blue)](https://golang.org)
[![Python](https://img.shields.io/badge/Python-3.10%2B-green)](https://python.org)
[![License](https://img.shields.io/badge/License-MIT%2FApache--2.0-purple)](LICENSE)

## What Problem Does This Solve?

Today's AI applications are forced to juggle **5-6 separate databases**:

| Current Stack                           | Pain Point                                   |
| --------------------------------------- | -------------------------------------------- |
| Vector DB (Pinecone, Weaviate, Milvus)  | Can't do relational joins or graph traversal |
| Graph DB (Neo4j, TigerGraph)            | No native vector similarity search           |
| Relational DB (PostgreSQL, MySQL)       | Embeddings are second-class, slow at scale   |
| Document Store (MongoDB, Elasticsearch) | No semantic meaning, no uncertainty          |
| Blob Storage (S3, MinIO)                | Disconnected from semantic layer             |
| Cache (Redis)                           | No semantic eviction                         |

**CDF replaces all of these with one engine** where semantic meaning, relationships, time, and uncertainty are first-class citizens.

---

## 🚀 Quick Start

### Prerequisites

| Tool   | Version          | Install Link       |
| ------ | ---------------- | ------------------ |
| Rust   | 1.75+            | https://rustup.rs  |
| Go     | 1.22+            | https://go.dev/dl  |
| Python | **3.10** (exact) | https://python.org |
| Docker | Latest           | https://docker.com |

> ⚠️ **Python 3.10 required**: The embedding service depends on `sentence-transformers` and `torch` which have compatibility constraints. Python 3.14+ is **not supported**.

### Automated Setup

```bash
# Windows PowerShell (as Administrator)
.\scripts\setup-all.ps1

# Linux / macOS
./scripts/setup-all.sh
```

The setup script will:

1. Check all prerequisites
2. Fetch Rust crates (`cargo fetch`)
3. Download Go modules (`go mod tidy`)
4. Install Python packages (`pip install -r requirements.txt`)
5. Pre-download the `all-MiniLM-L6-v2` embedding model

### Manual Setup

```bash
# 1. Rust dependencies
cargo fetch

# 2. Go dependencies
cd cmd/cdf-gateway && go mod tidy
cd ../cdf-router && go mod tidy
cd ../cdf-meta && go mod tidy
cd ../cdf-ctl && go mod tidy

# 3. Python dependencies (USE PYTHON 3.10)
pip install fastapi uvicorn sentence-transformers torch numpy scipy scikit-learn

# 4. Pre-download embedding model
python -c "from sentence_transformers import SentenceTransformer; SentenceTransformer('all-MiniLM-L6-v2')"
```

### Run the Full Stack

```bash
# Option 1: Docker Compose (recommended for exploration)
docker-compose up -d

# Option 2: Build from source (for development)
cargo build --workspace          # Rust storage + indexes
go build ./cmd/...              # Go control plane

# Start services individually
cargo run -p cdf-storage        # Storage node (port 50051)
go run ./cmd/cdf-gateway        # API gateway (port 8080)
go run ./cmd/cdf-router         # Query router (port 50050)
go run ./cmd/cdf-meta           # Meta service (port 50054)

# Python services (use Python 3.10 explicitly)
python3.10 ai-services/embed-service/main.py      # Port 8001
python3.10 ai-services/drift-detector/main.py     # Port 8002
```

### 🚀 Quick Demo (Docker Compose)

Run the full CDF stack with pre-loaded demo data in one command:

```bash
git clone https://github.com/cognitive-data-fabric/cdf.git
cd cdf
docker-compose up -d --build

# Wait 60 seconds for services + demo data seeding
curl http://localhost:8080/health
# → {"status": "healthy", "version": "0.1.0"}
```

**What starts:**

| Service        | Port     | Access                | Purpose                             |
| -------------- | -------- | --------------------- | ----------------------------------- |
| API Gateway    | **8080** | http://localhost:8080 | HTTP/gRPC entry point               |
| Query Router   | 50050    | internal              | CQL parsing, shard routing          |
| Meta Service   | 50054    | internal              | Schema registry, cluster membership |
| Storage Node 1 | 50051    | internal              | Shard range: 0-32767                |
| Storage Node 2 | 50052    | internal              | Shard range: 32768-65535            |
| Embed Service  | 8001     | internal              | `all-MiniLM-L6-v2` model            |
| Drift Detector | 8002     | internal              | Distribution shift detection        |
| Redis          | 6379     | internal              | Cache, pub/sub                      |
| MinIO Console  | **9001** | http://localhost:9001 | Blob/object store UI                |
| MinIO S3 API   | 9000     | internal              | Programmatic object access          |

**MinIO Login:** `minioadmin` / `minioadmin`

- **Auto-seeded demo data**: 8 ML papers with real embeddings + citation graph

### 🔧 Recent Fixes (May 2026)

If you pulled earlier and had build failures, these are now fixed on `main`:

| Issue                                                   | Fix                                                    |
| ------------------------------------------------------- | ------------------------------------------------------ |
| `go mod tidy` failed with invalid `raft-boltdb` version | Updated to valid version, committed `go.sum`           |
| Go Dockerfiles used `go mod tidy` without `go.sum`      | Now copy `go.sum` and use `go mod download`            |
| Rust `edition2024` not supported in `rust:1.75-alpine`  | Upgraded to `rust:1.86-alpine`                         |
| Storage Dockerfile only copied 2 crates                 | Now copies full `crates/` workspace                    |
| `cargo build` failed — missing `Result` type            | Added `pub type Result<T>` in `cdf-storage/src/lib.rs` |
| `engine.rs` — use of moved value                        | Fixed by extracting `row_id` before move               |
| `cdf-meta/main.go` — unused import                      | Removed `encoding/json`                                |
| Gateway hardcoded `localhost:50050`                     | Now reads `CDF_ROUTER_ADDR` from env                   |
| Missing `cdf-storage` binary target                     | Added `main.rs` + `[[bin]]` in `Cargo.toml`            |

### 📡 API Usage Examples

#### Health Check

```bash
curl http://localhost:8080/health
# → {"status":"healthy"}
```

#### Insert a Document (Scalar + Text)

```bash
curl -X POST http://localhost:8080/v1/insert \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "1",
    "table": "documents",
    "data": {
      "title": "Introduction to Vector Databases",
      "content": "Vector databases store high-dimensional embeddings...",
      "author": "Alice Smith",
      "published_year": 2024
    }
  }'
```

#### Insert with Embedding (Vector Search)

```bash
curl -X POST http://localhost:8080/v1/insert \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "2",
    "table": "papers",
    "data": {
      "title": "Attention Is All You Need",
      "abstract": "We propose a new simple network architecture...",
      "embedding": {
        "model_id": "all-MiniLM-L6-v2",
        "values": [0.1, 0.2, 0.3, ...]
      }
    }
  }'
```

#### Vector Search

```bash
curl -X POST http://localhost:8080/v1/search \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "3",
    "table": "papers",
    "vector": [0.1, 0.2, 0.3, 0.4],
    "top_k": 5,
    "threshold": 0.85
  }'
```

#### Insert with Graph Edge (Citation)

```bash
curl -X POST http://localhost:8080/v1/insert \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "4",
    "table": "citations",
    "data": {
      "from_paper": "paper-123",
      "to_paper": "paper-456",
      "edge_type": "cites",
      "weight": 1.0
    }
  }'
```

#### Query with CQL

```bash
curl -X POST http://localhost:8080/v1/query \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "5",
    "cql": "SELECT title FROM papers WHERE embedding SIMILAR TO [0.1,0.2,0.3] WITH THRESHOLD 0.85"
  }'
```

### 🔐 Access Control (RBAC + ACL)

CDF uses **Role-Based Access Control** (RBAC) combined with fine-grained **Access Control Lists** (ACL).

#### Role Hierarchy (higher implies lower)

| Role           | Level | What They Can Do                                   |
| -------------- | ----- | -------------------------------------------------- |
| **SuperAdmin** | 5     | Full system access, user management, cluster ops   |
| **Admin**      | 4     | Namespace management, schema evolution, ACL grants |
| **Developer**  | 3     | CRUD on tables, index creation, graph edges        |
| **Analyst**    | 2     | Read-only queries and vector search                |
| **Service**    | 1     | Service-to-service auth, no human login            |

#### ACL Granularity — Per-Resource Permissions

| Resource Type   | Permissions Available                         |
| --------------- | --------------------------------------------- |
| **Table**       | `Create`, `Read`, `Update`, `Delete`, `Admin` |
| **VectorIndex** | `Search`, `Insert`, `DeleteIndex`             |
| **Graph**       | `Traverse`, `ReadEdges`, `WriteEdges`         |
| **Schema**      | `Alter`, `Evolve`, `Register`                 |
| **Cluster**     | `NodeJoin`, `ShardRebalance`, `Backup`        |

ACL entries support **expiration** and **conditional rules** (time-based, IP-based).

#### Creating Users and Granting Access (Python SDK)

```python
from cdf_client import CdfClient, ApiKeyAuth

client = CdfClient("http://localhost:8080", auth=ApiKeyAuth("your-api-key"))

# 1. Create a user with a role
client.admin.create_user(
    username="alice",
    email="alice@example.com",
    password="secret",
    roles=["developer"]  # Options: superadmin, admin, developer, analyst, service
)

# 2. Grant ACL on a specific table
client.admin.grant_acl(
    principal_id="alice-id",
    resource_type="table",
    resource_id="documents",
    actions=["read", "write"],
    namespace="default"
)

# 3. Grant vector search-only access (Analyst pattern)
client.admin.grant_acl(
    principal_id="bob-id",
    resource_type="vector_index",
    resource_id="papers",
    actions=["search"],
    namespace="default"
)

# 4. Grant graph traversal access
client.admin.grant_acl(
    principal_id="alice-id",
    resource_type="graph",
    resource_id="citations",
    actions=["traverse", "read_edges"],
    namespace="default"
)

# 5. Grant with expiration (time-bound access)
client.admin.grant_acl(
    principal_id="contractor-id",
    resource_type="table",
    resource_id="sensitive_data",
    actions=["read"],
    namespace="default",
    expires_at="2026-12-31T23:59:59Z"
)

# 6. Revoke an ACL
client.admin.revoke_acl(
    principal_id="alice-id",
    resource_type="table",
    resource_id="documents",
    namespace="default"
)

# 7. List all ACLs for a user
acls = client.admin.list_acls(principal_id="alice-id")
for acl in acls:
    print(acl.resource_type, acl.resource_id, acl.actions)
```

#### Role-to-Default-Permission Mapping

| Role       | Default Table Access                  | Default Vector Access         | Default Graph Access              |
| ---------- | ------------------------------------- | ----------------------------- | --------------------------------- |
| SuperAdmin | `Create, Read, Update, Delete, Admin` | `Search, Insert, DeleteIndex` | `Traverse, ReadEdges, WriteEdges` |
| Admin      | `Create, Read, Update, Delete, Admin` | `Search, Insert`              | `Traverse, ReadEdges`             |
| Developer  | `Create, Read, Update, Delete`        | `Search, Insert`              | `Traverse, ReadEdges, WriteEdges` |
| Analyst    | `Read`                                | `Search`                      | `Traverse`                        |
| Service    | `Read`                                | `Search`                      | `Traverse`                        |

> **Note:** Explicit ACL grants override role defaults. A Developer with a revoked `Delete` ACL on `documents` cannot delete from that table, even though Developers normally can.

---

### 🔌 Consuming CDF in Your Applications

CDF exposes a **REST API** on port `8080` and provides a **Python SDK** (`cdf-client`). Any language can call the REST endpoints directly.

#### Install the Python SDK

```bash
pip install -e ./sdk/python   # from repo root
# or after publishing:
# pip install cdf-client
```

#### Authentication

```python
from cdf_client import CdfClient, ApiKeyAuth, JwtAuth

# Option 1: API Key (for services, scripts)
client = CdfClient("http://localhost:8080", auth=ApiKeyAuth("your-api-key"))

# Option 2: JWT (for users, auto-refreshes)
client = CdfClient("http://localhost:8080", auth=JwtAuth("alice", "secret"))
```

#### Data Operations (CdfClient)

| Method                               | What it does          | Example                                                 |
| ------------------------------------ | --------------------- | ------------------------------------------------------- |
| `health()`                           | Cluster health check  | `client.health()`                                       |
| `create_table(name, schema)`         | Create a table        | `client.create_table("documents", {"title": "string"})` |
| `list_tables(namespace)`             | List tables           | `client.list_tables("default")`                         |
| `insert(table, data)`                | Insert one row        | `client.insert("papers", {...})`                        |
| `batch_insert(table, rows)`          | Insert many rows      | `client.batch_insert("papers", [row1, row2])`           |
| `get(table, row_id)`                 | Fetch by ID           | `client.get("papers", "paper-123")`                     |
| `update(table, row_id, data)`        | Update row            | `client.update("papers", "paper-123", {...})`           |
| `delete(table, row_id)`              | Delete row            | `client.delete("papers", "paper-123")`                  |
| `query(cql)`                         | CQL query             | `client.query("SELECT * FROM papers")`                  |
| `search(table, vector, top_k)`       | Vector ANN search     | `client.search("papers", [0.1, 0.2], top_k=5)`          |
| `search_text(table, text)`           | Text → embed → search | `client.search_text("papers", "transformers")`          |
| `add_edge(from, to, type)`           | Graph edge            | `client.add_edge("A", "B", "cites")`                    |
| `traverse(start, edge_types, depth)` | Graph BFS             | `client.traverse("A", ["cites"], depth=2)`              |
| `neighbors(node, edge_type)`         | Immediate neighbors   | `client.neighbors("A", "cites")`                        |
| `subscribe(table)`                   | SSE stream of changes | `client.subscribe("papers")`                            |

#### Admin Operations (`client.admin`)

| Method                                                | What it does          |
| ----------------------------------------------------- | --------------------- |
| `admin.create_user(username, email, password, roles)` | Create user           |
| `admin.list_users(namespace, role)`                   | List/filter users     |
| `admin.update_user(user_id, roles, disabled)`         | Update user           |
| `admin.delete_user(user_id)`                          | Delete user           |
| `admin.create_api_key(name, user_id, roles)`          | Create API key        |
| `admin.revoke_api_key(key_id)`                        | Revoke API key        |
| `admin.grant_acl(...)`                                | Grant resource ACL    |
| `admin.revoke_acl(...)`                               | Revoke ACL            |
| `admin.list_acl(principal_id, resource_type)`         | Query ACLs            |
| `admin.create_namespace(name)`                        | Create namespace      |
| `admin.delete_namespace(name, force)`                 | Delete namespace      |
| `admin.cluster_status()`                              | Cluster health        |
| `admin.list_nodes()`                                  | List nodes            |
| `admin.drain_node(node_id)`                           | Drain for maintenance |
| `admin.rebalance_shards()`                            | Rebalance             |
| `admin.create_backup(name, tables)`                   | Backup                |
| `admin.restore_backup(backup_id)`                     | Restore               |
| `admin.list_audit_logs(...)`                          | Audit trail           |
| `admin.register_schema(name, schema)`                 | Register schema       |
| `admin.evolve_schema(name, changes)`                  | Evolve schema         |

#### Complete Application Example

```python
from cdf_client import CdfClient, ApiKeyAuth

# 1. Connect
client = CdfClient("http://localhost:8080", auth=ApiKeyAuth("dev-key"))

# 2. Create a table with mixed data types
client.create_table("research_papers", namespace="ml", schema={
    "title": {"type": "string", "required": True},
    "abstract": {"type": "text"},
    "embedding": {"type": "vector", "dimensions": 384, "metric": "cosine"},
    "confidence": {"type": "distribution"},
    "pdf_url": {"type": "blob_ref"},
    "citations": {"type": "graph_edges"}
})

# 3. Insert a paper with vector + graph + scalar
paper = client.insert("research_papers", {
    "title": "Attention Is All You Need",
    "abstract": "We propose a new simple network architecture...",
    "embedding": {"model_id": "all-MiniLM-L6-v2", "values": [0.1, 0.2, ...]},
    "confidence": {"mean": 0.95, "std_dev": 0.02},
    "pdf_url": "blob://a1b2c3d4...",  # MinIO content hash
    "year": 2017
}, namespace="ml")

paper_id = paper["row_id"]

# 4. Add citation edges (graph)
client.add_edge(paper_id, "paper-456", "cites", namespace="ml")
client.add_edge(paper_id, "paper-789", "cites", namespace="ml")

# 5. Semantic search
results = client.search("research_papers", vector=[0.1, 0.2, ...], top_k=5, namespace="ml")
for r in results:
    print(r["title"], r["score"])

# 6. Graph traversal — papers cited by this paper
cited = client.traverse(paper_id, edge_types=["cites"], depth=1, namespace="ml")

# 7. CQL query combining vector + graph + filter
response = client.query("""
    SELECT title, confidence
    FROM research_papers
    WHERE embedding SIMILAR TO :query WITH THRESHOLD 0.85
      AND year > 2020
      AND EXISTS PATH paper ->[cites]-> (paper.year > 2023)
""", params={"query": [0.1, 0.2, ...]})

# 8. Subscribe to real-time changes
for event in client.subscribe("research_papers", namespace="ml"):
    print(event.event, event.data)
```

#### REST API Endpoints (for non-Python clients)

| Method   | Endpoint                     | Body / Params                                                 | Description   |
| -------- | ---------------------------- | ------------------------------------------------------------- | ------------- |
| `GET`    | `/health`                    | —                                                             | Health check  |
| `POST`   | `/v1/insert`                 | `{"table", "data", "namespace"}`                              | Insert row    |
| `POST`   | `/v1/batch_insert`           | `{"table", "rows", "namespace"}`                              | Batch insert  |
| `GET`    | `/v1/rows/{ns}/{table}/{id}` | —                                                             | Get row       |
| `PUT`    | `/v1/update`                 | `{"table", "row_id", "data"}`                                 | Update row    |
| `DELETE` | `/v1/rows/{ns}/{table}/{id}` | —                                                             | Delete row    |
| `POST`   | `/v1/query`                  | `{"query", "params"}`                                         | CQL query     |
| `POST`   | `/v1/search`                 | `{"table", "vector", "top_k", "threshold"}`                   | Vector search |
| `POST`   | `/v1/search_text`            | `{"table", "text", "top_k"}`                                  | Text search   |
| `POST`   | `/v1/graph/edges`            | `{"from_id", "to_id", "edge_type"}`                           | Add edge      |
| `GET`    | `/v1/graph/traverse`         | `?start_id&depth&edge_types`                                  | Traverse      |
| `GET`    | `/v1/graph/neighbors`        | `?node_id&edge_type`                                          | Neighbors     |
| `GET`    | `/v1/subscribe`              | `?table&namespace&events`                                     | SSE stream    |
| `POST`   | `/v1/auth/token`             | `{"username", "password"}`                                    | Login         |
| `POST`   | `/v1/admin/users`            | `{"username", "email", "password", "roles"}`                  | Create user   |
| `POST`   | `/v1/admin/acl`              | `{"principal_id", "resource_type", "resource_id", "actions"}` | Grant ACL     |

---

### 🗂 Storing Different Data Types

CDF is **poly-modal** — one row can hold any combination of these value types:

| Data Type                             | How to Store                                             | Use Case                                | Stored In             |
| ------------------------------------- | -------------------------------------------------------- | --------------------------------------- | --------------------- |
| **Scalar** (int, float, string, bool) | Direct JSON value                                        | IDs, names, counts, flags               | LSM-tree inline       |
| **Text** (long documents)             | JSON string                                              | Articles, descriptions, logs            | LSM-tree inline       |
| **Embedding** (vector)                | `{"model_id": "...", "values": [...]}`                   | Semantic search, similarity             | HNSW index + LSM-tree |
| **Tensor** (multi-dimensional)        | `{"data": [...], "shape": [3, 224, 224]}`                | Image features, attention maps          | LSM-tree inline       |
| **Distribution** (uncertainty)        | `{"mean": 0.8, "std_dev": 0.1}`                          | AI confidence, probabilistic values     | LSM-tree inline       |
| **BlobRef** (large media)             | Content-hash string                                      | Images, videos, audio, PDFs             | MinIO (S3-compatible) |
| **GraphEdge** (relationship)          | `{"from_id": "...", "to_id": "...", "edge_type": "..."}` | Citations, social networks, hierarchies | CSR graph index       |

**BlobRef / MinIO workflow:**

1. Upload file to MinIO (via S3 API or console at http://localhost:9001)
2. Store the returned content hash in CDF as `BlobRef`
3. CDF fetches from MinIO on demand by content hash

### Your First Query (Python SDK)

```python
from cdf_client import CdfClient, ApiKeyAuth

# Connect
client = CdfClient("http://localhost:8080", auth=ApiKeyAuth("your-api-key"))

# Health check
print(client.health())

# Insert data
client.insert("documents", {
    "title": "Vector Databases",
    "content": "High-dimensional embeddings...",
    "embedding": {"model_id": "all-MiniLM-L6-v2", "values": [0.1, 0.2, ...]}
})

# Semantic search
results = client.search("documents", vector=[0.1, 0.2, ...], top_k=5)
for r in results:
    print(r["title"], r["score"])

# Admin: create user and grant ACL
client.admin.create_user("alice", "alice@example.com", "secret", roles=["developer"])
client.admin.grant_acl(
    principal_id="alice-id",
    resource_type="table",
    resource_id="documents",
    actions=["read", "write"],
    namespace="default"
)

# Graph traversal
paths = client.traverse("doc-123", edge_types=["cites"], depth=2)
```

### CLI

```bash
cdf-ctl query "SELECT title FROM documents WHERE embedding SIMILAR TO [0.1,0.2,...] WITH THRESHOLD 0.85"
cdf-ctl health
```

---

## 📁 Project Structure

```
cognitive-data-fabric/
├── crates/                    # Rust core engine
│   ├── cdf-common/           # Shared types, errors, serialization
│   ├── cdf-storage/          # LSM-tree storage engine with WAL
│   ├── cdf-vector/           # HNSW approximate nearest neighbor index
│   ├── cdf-graph/            # CSR graph adjacency index
│   ├── cdf-temporal/         # Bitemporal time-slice indexing
│   ├── cdf-prob/             # Probabilistic arithmetic
│   └── cdf-auth/             # RBAC, ACL, JWT authentication
├── cmd/                       # Go distributed services
│   ├── cdf-router/           # CQL parser and query planner
│   ├── cdf-gateway/          # HTTP/gRPC/WebSocket API gateway
│   ├── cdf-meta/             # Cluster membership and schema registry
│   └── cdf-ctl/              # Admin CLI tool
├── ai-services/               # Python ML services
│   ├── embed-service/        # Text/image/audio embedding generation
│   └── drift-detector/       # Distribution shift detection
├── sdk/                       # Client SDKs
│   └── python/               # Python SDK with admin support
├── proto/                     # gRPC protobuf definitions
├── deploy/                    # Docker, K8s, Terraform configs
└── docs/                      # Architecture and API documentation
```

---

## 🧠 Core Concepts

### Poly-Modal Values

One row can hold **any combination** of data types:

```rust
PolyValue::Embedding(Embedding { values: vec![0.1, 0.2, ...], model_id: "all-MiniLM-L6-v2" })
PolyValue::Distribution(Normal { mean: 0.8, std_dev: 0.1 })  // Uncertainty
PolyValue::Tensor(Tensor { data: vec![...], shape: vec![3, 224, 224] })  // Image feature map
PolyValue::BlobRef(ContentHash { hash: [...] })  // Reference to S3/MinIO
PolyValue::GraphEdge(GraphEdgeRef { from_id: ..., to_id: ..., edge_type: "cites" })
```

### Native Uncertainty

AI-generated values have confidence built in:

```sql
-- Filter by confidence
SELECT * FROM predictions
WHERE confidence CONFIDENCE > 0.95

-- Propagate uncertainty through arithmetic
SELECT price * quantity AS total  -- Automatically tracks error bounds
FROM orders
```

### Bitemporal Dimensions

Every record has two time axes:

- **Valid Time** — When the fact was true in the real world
- **Transaction Time** — When the system recorded it

```sql
-- Query as of last month
SELECT * FROM employees
AT TIME '2024-01-01'
WHERE department = 'Engineering'
```

### Semantic Graph

Vectors and relationships coexist:

```sql
-- Find papers similar to my query that cite recent work
SELECT paper.title, paper.confidence
FROM papers
WHERE embedding SIMILAR TO :query WITH THRESHOLD 0.85
  AND EXISTS PATH paper ->[cites]-> (paper.year > 2023)
```

---

## 🏗 Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed system design.

| Layer              | Technology | Responsibility                                      |
| ------------------ | ---------- | --------------------------------------------------- |
| **Storage Engine** | Rust       | LSM-tree, WAL, MemTable, segment files              |
| **Indexing**       | Rust       | HNSW (vectors), CSR (graphs), B-tree (temporal)     |
| **Query Router**   | Go         | CQL parsing, query planning, shard routing          |
| **API Gateway**    | Go         | HTTP/REST, gRPC, WebSocket, auth, rate limiting     |
| **Meta Service**   | Go         | Schema registry, cluster membership, Raft consensus |
| **AI Pipeline**    | Python     | Embedding generation, drift detection               |
| **Cache**          | Redis      | Semantic hot data, pub/sub                          |
| **Blob Store**     | MinIO      | Raw media, content-addressable                      |

---

## 📖 Documentation

- **[Architecture](docs/ARCHITECTURE.md)** — System design and data flow
- **[CQL Reference](docs/CQL.md)** — Cognitive Query Language specification
- **[API Docs](docs/API.md)** — REST and gRPC endpoint reference
- **[Contributing](CONTRIBUTING.md)** — Development setup and guidelines
- **[Rust Docs](https://docs.rs/cdf-common)** — Generated crate documentation

---

## 🛠 Development

### Building Components

```bash
# Rust workspace (all crates)
cargo build --workspace
cargo test --workspace

# Individual crates
cargo test -p cdf-storage
cargo test -p cdf-vector

# Go services
cd cmd/cdf-gateway && go build .
cd cmd/cdf-router && go build .
cd cmd/cdf-ctl && go build .

# Python services
pip install -e ".[dev]"
pytest ai-services/tests/

# Run benchmarks
cargo bench -p cdf-vector
```

### Project Scripts

| Script                    | Action                                 |
| ------------------------- | -------------------------------------- |
| `.\scripts\dev.ps1 start` | Start Docker environment (Windows)     |
| `./scripts/dev.sh start`  | Start Docker environment (Linux/macOS) |
| `dev.ps1 stop`            | Stop all containers                    |
| `dev.ps1 test`            | Run test suite                         |
| `dev.ps1 clean`           | Remove containers, volumes, prune      |

---

## 🎯 Roadmap

| Phase    | Status         | Features                                                       |
| -------- | -------------- | -------------------------------------------------------------- |
| **v0.1** | ✅ Complete    | Core types, storage engine, basic vector/graph indexes         |
| **v0.2** | 🚧 In Progress | Distributed transactions, replication, compaction              |
| **v0.3** | 📋 Planned     | GPU-accelerated vector search, multi-modal encoders            |
| **v0.4** | 📋 Planned     | Auto-embedding pipeline, semantic compression, context threads |
| **v1.0** | 📋 Planned     | Production hardening, cloud-native operator, managed service   |

---

## 🤝 Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Areas where help is especially appreciated:

- SIMD optimizations for distance computation (Rust)
- CUDA kernels for GPU vector search
- Additional distance metrics and quantization schemes
- CQL grammar improvements
- Kubernetes operator
- Connectors (LangChain, LlamaIndex, Haystack)

---

## 📄 License

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

---

## 💬 Community

- **Issues**: [GitHub Issues](https://github.com/cognitive-data-fabric/cdf/issues)
- **Discussions**: [GitHub Discussions](https://github.com/cognitive-data-fabric/cdf/discussions)
- **Discord**: [Join our server](https://discord.gg/cdf)

> Built with ❤️ by the Cognitive Data Fabric Team
