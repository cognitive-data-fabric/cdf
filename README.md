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
