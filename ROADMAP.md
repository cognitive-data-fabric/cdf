# CDF Roadmap

## v0.1.0 — Alpha (Current)

- [x] Core storage engine (LSM-tree with PolyValue)
- [x] HNSW vector index with cosine/euclidean/dot/manhattan
- [x] CSR graph index with BFS/DFS traversal
- [x] Bitemporal time dimensions (valid-time + transaction-time)
- [x] Probabilistic value types with confidence scoring
- [x] gRPC + REST gateway
- [x] CQL query language (SELECT, INSERT, vector SIMILAR TO, graph TRAVERSE)
- [x] RBAC with 5 roles + fine-grained ACL
- [x] JWT authentication with HMAC-SHA256
- [x] Python SDK
- [x] TypeScript SDK
- [x] Java SDK
- [x] JavaScript SDK
- [x] Docker Compose full-stack deployment
- [x] Demo data seeder (8 ML papers + embeddings + citation graph)

## v0.2.0 — Beta (Planned)

- [ ] Distributed consensus (Raft) for meta service
- [ ] Consistent hashing with virtual nodes for shard routing
- [ ] Vector-aware shard placement (LSH-based partitioning)
- [ ] Streaming queries via WebSocket
- [ ] Materialized views with incremental refresh
- [ ] CQL aggregations (COUNT, AVG, GROUP BY)
- [ ] CQL subqueries and CTEs
- [ ] gRPC streaming for large result sets
- [ ] Prometheus + Grafana dashboards
- [ ] Kubernetes Helm chart
- [ ] Data import/export tools (CSV, JSON, Parquet)
- [ ] Backup and restore

## v0.3.0 — Stable (Planned)

- [ ] Multi-region replication
- [ ] Automatic failover and leader election
- [ ] Query planner with cost-based optimization
- [ ] Secondary indexes (B-tree, bitmap)
- [ ] Full-text search integration
- [ ] Change data capture (CDC) streams
- [ ] Row-level security policies
- [ ] Audit logging
- [ ] Performance benchmarks suite
- [ ] Production hardening (fuzz testing, chaos engineering)

## v1.0.0 — GA (Future)

- [ ] SQL compatibility layer
- [ ] Foreign data wrappers (PostgreSQL, MySQL, MongoDB)
- [ ] GraphQL API
- [ ] Multi-tenancy with resource isolation
- [ ] Encryption at rest
- [ ] SOC 2 compliance
- [ ] Managed cloud offering
