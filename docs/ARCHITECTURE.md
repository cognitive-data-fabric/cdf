# Cognitive Data Fabric — Architecture

## Overview

CDF is a **polyglot, distributed, AI-native database** built on three core principles:

1. **Unified storage** — Vectors, graphs, scalars, tensors, and probabilistic values coexist in one row
2. **First-class time** — Every record carries bitemporal dimensions (valid-time + transaction-time)
3. **Native uncertainty** — Confidence and distributions are built into the type system, not bolted on

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              CLIENT LAYER                                │
│  (LangChain, LlamaIndex, Haystack, Custom Apps, Web UIs)                │
└──────────────────────────────────┬──────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────────┐
│                             API GATEWAY (Go)                             │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌─────────────────────┐  │
│  │ REST HTTP  │  │ gRPC       │  │ WebSocket  │  │ GraphQL (planned)   │  │
│  │ /v1/*      │  │ Streaming  │  │ Real-time  │  │                     │  │
│  └────────────┘  └────────────┘  └────────────┘  └─────────────────────┘  │
│                              Auth, Rate Limit, Metrics                   │
└──────────────────────────────────┬──────────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────────┐
│                            QUERY ROUTER (Go)                             │
│  ┌────────────────┐  ┌────────────────┐  ┌──────────────────────────┐   │
│  │ CQL Parser     │──▶│ Query Planner  │──▶│ Shard Router            │   │
│  │ ANTLR Grammar  │  │ Cost-based     │  │ Consistent Hash +       │   │
│  │                │  │ Optimization   │  │ Vector-aware routing      │   │
│  └────────────────┘  └────────────────┘  └──────────────────────────┘   │
└──────────────────────────────────┬──────────────────────────────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                    │
┌─────────────▼────────┐  ┌────────▼────────┐  ┌───────▼────────┐
│    META SERVICE      │  │  STORAGE NODE   │  │  STORAGE NODE  │
│       (Go)           │  │     (Rust)      │  │     (Rust)     │
│  ┌────────────────┐  │  │  ┌────────────┐  │  │  ┌────────────┐ │
│  │ Raft Consensus │  │  │  │ WAL / LSM  │  │  │  │ WAL / LSM  │ │
│  │ Schema Registry│  │  │  │ MemTable   │  │  │  │ MemTable   │ │
│  │ Cluster Members│  │  │  │ Segments   │  │  │  │ Segments   │ │
│  └────────────────┘  │  │  └────────────┘  │  │  └────────────┘ │
└──────────────────────┘  │  ┌────────────┐  │  │  ┌────────────┐ │
                          │  │ Vector Idx │  │  │  │ Vector Idx │ │
                          │  │ (HNSW)     │  │  │  │ (HNSW)     │ │
                          │  ├────────────┤  │  │  ├────────────┤ │
                          │  │ Graph Idx  │  │  │  │ Graph Idx  │ │
                          │  │ (CSR)      │  │  │  │ (CSR)      │ │
                          │  ├────────────┤  │  │  ├────────────┤ │
                          │  │ Temporal   │  │  │  │ Temporal   │ │
                          │  │ Index      │  │  │  │ Index      │ │
                          │  └────────────┘  │  │  └────────────┘ │
                          └──────────────────┘  └─────────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                    │
┌─────────────▼────────┐  ┌───────▼────────┐  ┌───────▼────────┐
│  EMBED SERVICE (Py)  │  │ DRIFT (Py)     │  │ REDIS / MINIO  │
│  Sentence-Transformers│  │ MMD + Wasser.  │  │ Cache / Blobs  │
│  Batch, Cache, Models │  │ Distribution   │  │                │
└──────────────────────┘  └────────────────┘  └────────────────┘
```

---

## Data Model

### Poly-Modal Row

Every row in CDF is a `PolyRow` containing a map of column names to `PolyValue`:

```rust
PolyRow {
    id: FabricId,                              // UUID v4
    values: HashMap<String, PolyValue>,
    temporal: TemporalBounds,                   // bitemporal
    version: u64,                              // optimistic locking
}
```

### PolyValue Variants

| Variant        | Use Case                | Example                        |
| -------------- | ----------------------- | ------------------------------ |
| `Scalar(f64)`  | Numbers, prices         | `42.5`                         |
| `Text(String)` | Documents, queries      | `"Hello world"`                |
| `Embedding`    | Semantic vectors        | 384-dim sentence embedding     |
| `Tensor`       | Feature maps, attention | `[3, 224, 224]` image features |
| `Distribution` | Uncertain values        | `Normal(mean=0.8, std=0.1)`    |
| `BlobRef`      | Raw media               | Content-hash → MinIO           |
| `GraphEdge`    | Relationships           | `paperA --cites--> paperB`     |

### Schema Evolution

Tables use **dynamic schemas** with version tracking:

```rust
TableSchema {
    name: "documents",
    version: 3,
    columns: [
        ColumnDef { name: "title", dtype: DataType::String, nullable: false },
        ColumnDef { name: "embedding", dtype: DataType::Vector { dimensions: 384, metric: "cosine" }},
        ColumnDef { name: "confidence", dtype: DataType::Distribution { family: "beta" }},
    ],
    indexes: [IndexDef { index_type: IndexType::Hnsw, columns: ["embedding"] }],
}
```

---

## Storage Engine

### Write Path

```
Client → Gateway → Router → Storage Node
                              │
                              ├── 1. Serialize to WAL (durability)
                              ├── 2. Insert into MemTable (sorted BTreeMap)
                              ├── 3. Update indexes (HNSW, CSR, Temporal)
                              └── 4. If MemTable full → flush to Segment
```

### Segment Format (SSTable-style)

```
┌─────────────────────────────────────────────────────────────┐
│  Segment Header                                               │
│  - Magic: "CDFSEG"                                           │
│  - Version: 1                                                │
│  - Row count, level, timestamp                               │
├─────────────────────────────────────────────────────────────┤
│  Index Block                                                  │
│  - Sparse key offsets (every N keys)                         │
│  - Bloom filter for key membership                           │
├─────────────────────────────────────────────────────────────┤
│  Data Blocks (sorted by key)                                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│  │ Key1 | Val1 │ │ Key2 | Val2 │ │ Key3 | Val3 │            │
│  └─────────────┘ └─────────────┘ └─────────────┘            │
├─────────────────────────────────────────────────────────────┤
│  Footer                                                       │
│  - Offset to index block                                     │
│  - Statistics (min/max key, avg value size)                   │
└─────────────────────────────────────────────────────────────┘
```

### Compaction

Level-based compaction (RocksDB-style):

| Level | Max Size | Trigger           |
| ----- | -------- | ----------------- |
| L0    | 64MB     | MemTable flush    |
| L1    | 256MB    | L0 compaction     |
| L2    | 1GB      | L1 compaction     |
| L3+   | 4GB      | Tiered compaction |

---

## Indexing

### Vector Index (HNSW)

```rust
HnswIndex {
    params: HnswParams {
        m: 16,              // max neighbors per node
        ef_construction: 200, // search depth during build
        ml: 1.0 / ln(2),    // level generation factor
    },
    nodes: HashMap<FabricId, HnswNode>,
    entry_point: Option<FabricId>,
}
```

**Search Complexity**: O(log N) average, O(N) worst-case

### Graph Index (CSR)

```rust
CsrGraph {
    nodes: HashMap<FabricId, Vec<String>>,     // labels
    edges: Vec<GraphEdgeRef>,                    // all edges
    row_ptr: Vec<usize>,                         // offsets
    col_idx: Vec<FabricId>,                      // destinations
}
```

**Traversal**: BFS/DFS with optional edge-type filtering

### Temporal Index

```rust
TemporalIndex {
    valid_time_index: BTreeMap<Timestamp, Vec<u64>>,
    transaction_index: BTreeMap<Timestamp, Vec<u64>>,
}
```

**Query Types**:

- `TemporalQuery::Current` — latest version
- `TemporalQuery::AsOfValid(t)` — valid at time t
- `TemporalQuery::Bitemporal { valid, transaction }` — full slice

---

## Query Processing

### CQL (Cognitive Query Language)

```sql
SELECT projection
FROM source
[WHERE condition]
[WITH vector_clause]
[AT TIME temporal_expr]
[ORDER BY order_list]
[LIMIT limit];
```

**Probabilistic Predicate:**

```sql
WHERE confidence CONFIDENCE > 0.95
WHERE prediction EXPECTED > 0.5
```

### Query Planner

1. **Parse** — ANTLR grammar → AST
2. **Validate** — Schema check, type inference
3. **Optimize** — Cost-based: selectivity, index availability
4. **Plan** — Physical operators (IndexScan, VectorSearch, GraphTraverse)
5. **Route** — Shard determination (hash + vector locality)
6. **Execute** — Fan-out to storage nodes, collect results
7. **Rerank** — Combine vector similarity + graph distance + scalar filters

---

## Distributed Architecture

### Shard Map

```go
type ShardMap struct {
    Version      uint64
    Partitions   []Partition
    Replicas     int
    Consistency  ConsistencyLevel

    // Semantic-aware: similar vectors hash to same shard
    EmbeddingRouter *ConsistentHashRing
}

type Partition struct {
    ShardID    uint64
    RangeStart uint64      // hash range
    RangeEnd   uint64
    Primary    NodeInfo
    Replicas   []NodeInfo
}
```

### Consistency Levels

| Level              | Behavior                | Use Case                |
| ------------------ | ----------------------- | ----------------------- |
| `Eventual`         | Fastest, may read stale | Analytics, non-critical |
| `Session`          | Read your writes        | User sessions           |
| `BoundedStaleness` | Max lag in ms           | Near-real-time          |
| `Strong`           | Quorum read/write       | Financial, critical     |

### Raft Consensus (Meta Service)

- Leader election for schema changes
- Log replication for membership updates
- Snapshotting for fast recovery

---

## AI Pipeline

### Embedding Service

```python
# On ingestion: auto-generate embeddings
POST /embed
{
    "texts": ["query text"],
    "model_id": "all-MiniLM-L6-v2",
    "normalize": true
}

# Response
{
    "embeddings": [[0.1, 0.2, ...]],
    "dimensions": 384,
    "processing_time_ms": 12.5
}
```

### Drift Detection

```python
# Register baseline
POST /register-reference
{
    "concept_id": "sports",
    "embeddings": [[...], [...], ...]
}

# Check current batch for drift
POST /check-drift
{
    "concept_id": "sports",
    "embeddings": [[...], [...]]
}

# Response
{
    "drift_detected": true,
    "drift_score": 0.82,
    "p_value": 0.001,
    "method": "mmd+wasserstein"
}
```

**Algorithms:**

- **MMD** (Maximum Mean Discrepancy) — distribution comparison
- **Wasserstein Distance** — sliced optimal transport
- **Bootstrap p-value** — statistical significance

---

## Deployment

### Docker Compose (Development)

```bash
docker-compose up -d

# Services started:
# - 2x Storage Node (Rust)
# - Router (Go)
# - Gateway (Go)
# - Meta (Go)
# - Embed Service (Python)
# - Drift Detector (Python)
# - Redis
# - MinIO
```

### Kubernetes (Production)

See `deploy/kubernetes/` for:

- StatefulSet for storage nodes (persistent volumes)
- Deployment for stateless services
- Service mesh (Istio/Linkerd)
- Prometheus + Grafana monitoring
- Cert-manager for TLS

---

## Performance Targets

| Metric                         | Target     | Current   |
| ------------------------------ | ---------- | --------- |
| Vector search latency (p99)    | < 10ms     | ~15ms     |
| Write throughput               | 100K ops/s | 50K ops/s |
| Graph traversal (3-hop)        | < 50ms     | ~80ms     |
| Temporal query                 | < 20ms     | ~25ms     |
| Embedding generation           | < 20ms     | ~15ms     |
| Drift detection (1000 samples) | < 100ms    | ~120ms    |

---

## Security

### Authentication

CDF supports two authentication mechanisms:

| Method      | Use Case                    | Details                                   |
| ----------- | --------------------------- | ----------------------------------------- |
| **API Key** | Service-to-service, scripts | Static key with `X-API-Key` header        |
| **JWT**     | User sessions, web apps     | HMAC-SHA256 signed, 1h TTL, refresh token |

JWT tokens include claims for:

- `sub` — principal ID
- `roles` — assigned roles
- `namespaces` — accessible namespaces
- `mfa_verified` — MFA completion status

### Authorization (RBAC + ACL)

**Role Hierarchy** (higher implies lower):

1. **SuperAdmin** — Full system access, user management, cluster ops
2. **Admin** — Namespace management, schema evolution, ACL grants
3. **Developer** — CRUD on tables, index creation, graph edges
4. **Analyst** — Read-only queries and vector search
5. **Service** — Service-to-service auth, no human login

**ACL Granularity** — Per-resource permissions:

- **Table**: Create, Read, Update, Delete, Admin
- **VectorIndex**: Search, Insert, DeleteIndex
- **Graph**: Traverse, ReadEdges, WriteEdges
- **Schema**: Alter, Evolve, Register
- **Cluster**: NodeJoin, ShardRebalance, Backup

ACL entries support expiration and conditional rules (time-based, IP-based).

### Audit Logging

All authenticated requests are logged with:

- Principal ID, timestamp, IP address
- Action, resource, allowed/denied
- Reason for denial (if applicable)

### Transport & Storage

- **TLS** — All inter-service communication
- **mTLS** — Service mesh identity
- **Encryption at Rest** — AES-256 for segment files

---

## Monitoring

| Component     | Tool                      | Metrics                      |
| ------------- | ------------------------- | ---------------------------- |
| Rust Services | Prometheus + tracing      | Latency, throughput, errors  |
| Go Services   | Prometheus + pprof        | Goroutines, GC, RPC latency  |
| Python        | OpenTelemetry + structlog | Model inference, queue depth |
| Cluster       | Grafana dashboards        | Node health, shard balance   |

---

## Related Work

| System                    | What CDF Does Differently              |
| ------------------------- | -------------------------------------- |
| **Pinecone / Weaviate**   | Adds graph + temporal + probabilistic  |
| **Neo4j**                 | Adds vector ANN + uncertainty          |
| **PostgreSQL + pgvector** | Native (not bolted-on), distributed    |
| **DuckDB**                | Distributed, AI-native, not OLAP-only  |
| **Spanner**               | Adds semantic + uncertainty dimensions |

---

## Glossary

| Term               | Definition                                                        |
| ------------------ | ----------------------------------------------------------------- |
| **PolyValue**      | Unified type system for scalar/vector/tensor/prob/blob/graph      |
| **TemporalBounds** | Valid-time + transaction-time for bitemporal queries              |
| **FabricId**       | UUIDv4 identifier used across all components                      |
| **CQL**            | Cognitive Query Language — hybrid SQL + vector + graph            |
| **HNSW**           | Hierarchical Navigable Small World — approximate nearest neighbor |
| **CSR**            | Compressed Sparse Row — memory-efficient graph format             |
| **WAL**            | Write-Ahead Log — crash recovery mechanism                        |
| **LSM**            | Log-Structured Merge — storage engine design                      |
