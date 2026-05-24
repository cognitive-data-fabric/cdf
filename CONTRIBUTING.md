# Contributing to Cognitive Data Fabric

Thank you for your interest in contributing to CDF! This document will help you get started.

---

## Development Setup

### Prerequisites

| Tool     | Version | Purpose                  |
| -------- | ------- | ------------------------ |
| Rust     | 1.75+   | Storage engine, indexes  |
| Go       | 1.22+   | Distributed services     |
| Python   | 3.10+   | AI/ML pipeline           |
| Docker   | Latest  | Containerized deployment |
| Protobuf | 3.20+   | gRPC code generation     |

### Clone and Build

```bash
git clone https://github.com/cognitive-data-fabric/cdf.git
cd cdf
```

#### Step 1: Rust

```bash
rustup update
cargo fetch        # Download dependencies
cargo build --workspace
```

> Note: The workspace currently has some compilation errors in `cdf-vector` (f32 Ord issue) and `cdf-storage` (Result type). See [Troubleshooting](#troubleshooting).

#### Step 2: Go

```bash
cd cmd/cdf-gateway && go mod tidy && go build .
cd ../cdf-router && go mod tidy && go build .
cd ../cdf-meta && go mod tidy && go build .
cd ../cdf-ctl && go mod tidy && go build .
```

#### Step 3: Python (CRITICAL — Use Python 3.10)

```bash
# Verify you have Python 3.10
python3.10 --version   # or python --version

# Install dependencies
pip install fastapi uvicorn sentence-transformers torch numpy scipy scikit-learn structlog

# Pre-download the embedding model
python3.10 -c "from sentence_transformers import SentenceTransformer; SentenceTransformer('all-MiniLM-L6-v2')"
```

> ⚠️ **Python 3.10 is required**. `sentence-transformers` and `torch` have binary compatibility constraints. Python 3.14+ will fail with `numpy.dtype size changed` or `torchvision::nms` errors.

### Verify Installation

```bash
# Rust (some crates may have warnings)
cargo test -p cdf-common
cargo test -p cdf-graph

# Go
go test ./cmd/...

# Python
python3.10 -c "from sentence_transformers import SentenceTransformer; print('OK')"

# Full stack
docker-compose up -d
curl http://localhost:8080/health
```

---

## Project Structure

```
crates/          # Rust library crates
cmd/             # Go binaries (main packages)
ai-services/     # Python FastAPI services
proto/           # gRPC definitions
docs/            # Documentation
deploy/          # Deployment configs
scripts/         # Development helpers
```

### Code Organization

| Directory                    | Language | Description                         |
| ---------------------------- | -------- | ----------------------------------- |
| `crates/cdf-common`          | Rust     | Types shared across all Rust crates |
| `crates/cdf-storage`         | Rust     | LSM-tree, WAL, MemTable, segments   |
| `crates/cdf-vector`          | Rust     | HNSW index                          |
| `crates/cdf-graph`           | Rust     | CSR graph                           |
| `crates/cdf-temporal`        | Rust     | Temporal indexing                   |
| `crates/cdf-prob`            | Rust     | Probabilistic arithmetic            |
| `cmd/cdf-router`             | Go       | Query routing and planning          |
| `cmd/cdf-gateway`            | Go       | HTTP/gRPC API                       |
| `cmd/cdf-meta`               | Go       | Cluster metadata                    |
| `cmd/cdf-ctl`                | Go       | CLI tool                            |
| `ai-services/embed-service`  | Python   | Embedding generation                |
| `ai-services/drift-detector` | Python   | Drift detection                     |

---

## Development Workflow

### 1. Fork and Branch

```bash
git checkout -b feature/your-feature-name
```

### 2. Make Changes

- Follow existing code style
- Add tests for new functionality
- Update documentation

### 3. Test

```bash
# Rust
cargo test --workspace
cargo clippy --workspace

# Go
gofmt -w .
go vet ./...
go test ./...

# Python
black .
ruff check .
mypy ai-services/
pytest
```

### 4. Commit

We follow conventional commits:

```
feat: add HNSW index persistence
fix: resolve WAL rotation race condition
docs: update API reference for graph queries
test: add temporal index benchmarks
```

### 5. Push and PR

```bash
git push origin feature/your-feature-name
```

Open a Pull Request with:

- Clear description of changes
- Link to related issue(s)
- Test results
- Documentation updates

---

## Code Style

### Rust

- Follow `rustfmt` defaults
- Use `clippy` warnings as errors
- Document public APIs with `///`
- Use `thiserror` for error types
- Prefer `?` over `match` for error propagation

```rust
// Good
pub fn process(data: &[u8]) -> Result<Vec<u8>, CdfError> {
    let parsed = parse(data)?;
    validate(&parsed)?;
    transform(parsed)
}

// Avoid
pub fn process(data: &[u8]) -> Result<Vec<u8>, CdfError> {
    match parse(data) {
        Ok(parsed) => match validate(&parsed) {
            Ok(_) => transform(parsed),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}
```

### Go

- Use `gofmt`
- Keep functions under 60 lines when possible
- Use interfaces for testability
- Handle errors explicitly

```go
// Good
func (s *Service) Process(ctx context.Context, req *Request) (*Response, error) {
    if err := validate(req); err != nil {
        return nil, fmt.Errorf("invalid request: %w", err)
    }
    return s.processInternal(ctx, req)
}

// Avoid
func (s *Service) Process(ctx context.Context, req *Request) *Response {
    // Implicit error handling
    return s.processInternal(ctx, req)
}
```

### Python

- Use `black` for formatting (line length 100)
- Use `ruff` for linting
- Type hints required for public APIs
- Use `structlog` for structured logging

```python
# Good
from typing import List

def process_embeddings(texts: List[str]) -> List[List[float]]:
    logger.info("processing_embeddings", count=len(texts))
    return [embed(t) for t in texts]

# Avoid
def process_embeddings(texts):
    print("processing", len(texts))
    return [embed(t) for t in texts]
```

---

## Testing

### Unit Tests

```bash
# Rust
cargo test -p cdf-storage

# Go
go test -v ./cmd/cdf-router

# Python
pytest ai-services/tests/test_embed.py -v
```

### Integration Tests

```bash
# Start full stack
docker-compose up -d

# Run integration suite
cargo test --test integration
pytest ai-services/tests/integration/
```

### Benchmarks

```bash
# Vector search benchmarks
cargo bench -p cdf-vector

# Storage engine benchmarks
cargo bench -p cdf-storage
```

---

## Areas for Contribution

### High Priority

| Area               | Description                       | Skills              |
| ------------------ | --------------------------------- | ------------------- |
| SIMD Optimizations | AVX2/NEON distance functions      | Rust, Assembly      |
| GPU Acceleration   | CUDA kernels for HNSW             | C++, CUDA           |
| CQL Parser         | Full ANTLR grammar implementation | Parser theory       |
| Raft Consensus     | Complete etcd-style consensus     | Distributed systems |

### Medium Priority

| Area                        | Description                     |
| --------------------------- | ------------------------------- |
| Kubernetes Operator         | Helm charts, CRDs, operators    |
| Connectors                  | LangChain, LlamaIndex, Haystack |
| Additional Distance Metrics | Hamming, Jaccard, Mahalanobis   |
| Product Quantization        | Memory-efficient vector storage |

### Documentation

- API examples in more languages
- Tutorials for common use cases
- Benchmark comparisons with other systems
- Architecture decision records (ADRs)

---

## Getting Help

- **Discord**: [Join our community](https://discord.gg/cdf)
- **GitHub Discussions**: For design questions and ideas
- **GitHub Issues**: For bugs and feature requests

---

## Code of Conduct

Be respectful, inclusive, and constructive. We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

---

## Troubleshooting

### Python: `numpy.dtype size changed` or `torchvision::nms` errors

**Cause**: Using Python 3.14+ which is incompatible with `sentence-transformers` and `torch`.

**Fix**: Use Python 3.10 explicitly:

```bash
python3.10 -m pip install sentence-transformers torch
python3.10 ai-services/embed-service/main.py
```

### Python: `No module named 'PIL'`

**Cause**: Pillow installed in wrong Python environment.

**Fix**: Install Pillow for Python 3.10:

```bash
python3.10 -m pip install Pillow
```

### Rust: `f32 does not implement Ord` in `cdf-vector`

**Cause**: BinaryHeap requires `Ord` trait, but `f32` only has `PartialOrd`.

**Fix**: Wrap distances in a custom type implementing `Ord`, or use `ordered-float` crate.

### Rust: `cannot find type Result in crate root`

**Cause**: `cdf-storage` and `cdf-vector` crates don't re-export `Result`.

**Fix**: Add `pub type Result<T> = std::result::Result<T, CdfError>` to each crate's `lib.rs`.

### Go: Module not found

**Cause**: Go modules not initialized.

**Fix**: Run `go mod tidy` in each `cmd/` subdirectory.

---

## License

By contributing, you agree that your contributions will be dual-licensed under MIT OR Apache-2.0.
