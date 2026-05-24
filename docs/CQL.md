# CQL — Cognitive Query Language Reference

CQL is the query language for the Cognitive Data Fabric. It extends SQL with first-class support for vector similarity, graph traversal, temporal queries, and probabilistic predicates.

---

## Syntax Overview

```ebnf
query ::= select_clause
          from_clause
          [where_clause]
          [with_clause]
          [at_time_clause]
          [order_by_clause]
          [limit_clause]
          ;

select_clause ::= "SELECT" ["DISTINCT"] projection
projection    ::= "*" | column_list | aggregate_list
column_list   ::= column_expr ("," column_expr)*
column_expr   ::= column_ref ["AS" identifier]
                | function_call ["AS" identifier]
                | vector_expr
                | path_expr
;

from_clause   ::= "FROM" table_ref [table_alias]
                | "FROM" "(" subquery ")" table_alias
                | "FROM" graph_pattern
;

where_clause  ::= "WHERE" condition
condition     ::= boolean_expr
;

with_clause   ::= "WITH" vector_clause | context_clause
;

at_time_clause ::= "AT" "TIME" temporal_expr
;

order_by_clause ::= "ORDER" "BY" sort_expr ("," sort_expr)*
sort_expr     ::= column_ref ["ASC" | "DESC"]
                | vector_distance_expr
;

limit_clause  ::= "LIMIT" integer ["OFFSET" integer]
;
```

---

## Core SQL Features

### SELECT with standard predicates

```sql
SELECT id, title, author, published_year
FROM papers
WHERE published_year > 2020 AND journal = 'Nature'
ORDER BY published_year DESC
LIMIT 100;
```

### Aggregations

```sql
SELECT department, AVG(salary), COUNT(*)
FROM employees
GROUP BY department
HAVING COUNT(*) > 10;
```

### JOINs (cross-table)

```sql
SELECT p.title, a.name
FROM papers p
JOIN authors a ON p.author_id = a.id
WHERE p.embedding SIMILAR TO :query;
```

---

## Vector Operations

### Similarity Search

```sql
SELECT title, abstract
FROM papers
WHERE embedding SIMILAR TO :query_embedding WITH THRESHOLD 0.85;
```

- `embedding` — the vector column to search
- `:query_embedding` — parameter binding for query vector
- `THRESHOLD` — minimum similarity (0.0 to 1.0)

### Top-K Search

```sql
SELECT title, embedding
FROM papers
WHERE embedding SIMILAR TO :query WITH THRESHOLD 0.80
ORDER BY embedding_distance ASC
LIMIT 10;
```

### Related Concepts (Graph + Vector)

```sql
SELECT concept.name, concept.definition
FROM concepts
WHERE embedding RELATED TO :concept_embedding WITH DEPTH 3;
```

- `RELATED TO` — walks graph edges, computing vector similarity at each hop
- `DEPTH` — maximum graph traversal depth

### Vector Distance Functions

```sql
-- Explicit distance in SELECT
SELECT title, cosine_distance(embedding, :query) AS dist
FROM papers
WHERE dist < 0.3;

-- Supported metrics
euclidean_distance(a, b)
manhattan_distance(a, b)
dot_product(a, b)
cosine_similarity(a, b)  -- returns 0-1
```

---

## Graph Traversal

### Edge Navigation

```sql
-- Direct neighbors
SELECT friend.name
FROM people p
WHERE p.id = :user_id
  AND EXISTS PATH p ->[:friends_with]-> friend;

-- Multi-hop with conditions
SELECT paper.title
FROM papers source
WHERE source.id = :paper_id
  AND EXISTS PATH source ->[:cites]-> [:cites]-> (paper.year > 2020);

-- Named paths
SELECT path
FROM papers
WHERE REACHABLE FROM :start_id WITHIN 3 HOPS
  AND path.edge_types = ['cites', 'references'];
```

### Path Functions

```sql
-- Shortest path
SELECT shortest_path(from_id, to_id) AS path
FROM graph;

-- Path length
SELECT p.title, path_length(:user, p) AS distance
FROM papers p
WHERE distance <= 2;
```

---

## Temporal Queries

### Time-Travel

```sql
-- As of a specific date
SELECT * FROM employees
AT TIME '2024-01-01'
WHERE department = 'Engineering';

-- As of transaction time
SELECT * FROM stock_prices
AT TRANSACTION TIME '2024-06-15T14:30:00Z';

-- Full bitemporal
SELECT * FROM contracts
AT TIME '2024-01-01' TRANSACTION TIME '2024-06-01';
```

### Temporal Predicates

```sql
-- Records valid during a range
SELECT * FROM employees
WHERE valid_during('2023-01-01', '2024-01-01');

-- Records that changed recently
SELECT * FROM predictions
WHERE transaction_time > now() - interval '1 hour';
```

---

## Probabilistic Queries

### Confidence Filtering

```sql
-- Filter by confidence level
SELECT prediction, confidence
FROM model_outputs
WHERE confidence CONFIDENCE > 0.95;

-- Expected value comparison
SELECT *
FROM sensor_readings
WHERE temperature EXPECTED > 100.0;

-- Range queries with uncertainty
SELECT *
FROM measurements
WHERE measurement OVERLAPS interval(50.0, 60.0) WITH CONFIDENCE 0.90;
```

### Probabilistic Aggregation

```sql
-- Weighted average with uncertainty propagation
SELECT sensor_id, weighted_avg(reading, confidence_weight) AS avg_reading
FROM sensors
GROUP BY sensor_id;

-- Aggregate confidence
SELECT AVG(confidence) AS avg_confidence
FROM predictions
WHERE model_id = 'gpt-4';
```

---

## Context and Sessions

### Context-Aware Queries

```sql
-- Within a conversation context
SELECT answer
FROM knowledge_base
WITH CONTEXT :conversation_id
WHERE question SIMILAR TO :user_query;

-- Semantic compression of old context
SELECT compressed_context
FROM session_contexts
WHERE session_id = :session_id
  AND context_age > interval '10 minutes';
```

---

## Data Modification

### INSERT

```sql
INSERT INTO papers (id, title, embedding, confidence)
VALUES (
    'paper-001',
    'Attention Is All You Need',
    embedding([0.1, 0.2, ...]),
    normal(0.95, 0.05)
);
```

### UPSERT

```sql
UPSERT INTO papers (id, title, embedding)
VALUES ('paper-001', 'Updated Title', embedding([...]))
ON CONFLICT (id) DO UPDATE;
```

### DELETE (Logical)

```sql
-- Logical delete: closes valid-time range
DELETE FROM papers WHERE id = 'paper-001';

-- Physical delete: removes all versions
DELETE ALL VERSIONS FROM papers WHERE id = 'paper-001';
```

---

## Data Definition

### CREATE TABLE

```sql
CREATE TABLE papers (
    id STRING PRIMARY KEY,
    title STRING NOT NULL,
    abstract TEXT,
    embedding VECTOR(384, metric='cosine'),
    citations GRAPH_EDGE('cites'),
    published_date TEMPORAL,
    confidence DISTRIBUTION('beta'),
    raw_pdf BLOB_REF,
    metadata JSON
);

-- With indexes
CREATE INDEX idx_embedding ON papers USING hnsw(embedding);
CREATE INDEX idx_temporal ON papers USING btree(published_date);
CREATE INDEX idx_graph ON papers USING graph(citations);
```

### ALTER TABLE

```sql
ALTER TABLE papers ADD COLUMN impact_score FLOAT;
ALTER TABLE papers DROP COLUMN deprecated_field;
```

---

## Functions Reference

### Vector Functions

| Function                  | Description             | Example                              |
| ------------------------- | ----------------------- | ------------------------------------ |
| `embedding(values[])`     | Cast array to embedding | `embedding([0.1, 0.2])`              |
| `cosine_similarity(a, b)` | Cosine similarity       | `cosine_similarity(e, :q) > 0.9`     |
| `l2_distance(a, b)`       | Euclidean distance      | `l2_distance(e, :q) < 0.5`           |
| `normalize(e)`            | L2 normalize            | `normalize(embedding) SIMILAR TO :q` |

### Graph Functions

| Function                     | Description      | Example                   |
| ---------------------------- | ---------------- | ------------------------- |
| `shortest_path(from, to)`    | Shortest path    | `shortest_path(a, b)`     |
| `path_length(from, to)`      | Path length      | `path_length(a, b) <= 3`  |
| `neighbors(node, edge_type)` | Direct neighbors | `neighbors(n, 'friends')` |

### Temporal Functions

| Function            | Description            | Example                              |
| ------------------- | ---------------------- | ------------------------------------ |
| `now()`             | Current timestamp      | `WHERE t > now() - interval '1d'`    |
| `valid_at(t)`       | Check valid time       | `WHERE valid_at('2024-01-01')`       |
| `transaction_at(t)` | Check transaction time | `WHERE transaction_at('2024-06-01')` |

### Probabilistic Functions

| Function                                  | Description         | Example                |
| ----------------------------------------- | ------------------- | ---------------------- |
| `normal(mean, std)`                       | Normal distribution | `normal(10.0, 2.0)`    |
| `beta(alpha, beta)`                       | Beta distribution   | `beta(2.0, 3.0)`       |
| `confidence_interval(lower, upper, conf)` | CI                  | `ci(0.1, 0.5, 0.95)`   |
| `expected(v)`                             | Expected value      | `expected(confidence)` |

---

## Examples

### Semantic RAG Pipeline

```sql
-- Retrieve relevant documents with graph context
SELECT doc.content, doc.confidence,
       shortest_path(doc, :root_concept) AS semantic_path
FROM documents doc
WHERE doc.embedding SIMILAR TO :query WITH THRESHOLD 0.80
  AND doc.confidence CONFIDENCE > 0.90
  AND EXISTS PATH doc ->[:related_to]->* (:root_concept)
ORDER BY cosine_similarity(doc.embedding, :query) DESC,
         doc.confidence EXPECTED DESC
LIMIT 5;
```

### Time-Series Anomaly Detection

```sql
-- Find sensor readings with unusual uncertainty
SELECT sensor_id, reading, confidence
FROM sensor_readings
AT TIME '2024-01-01'
WHERE reading.confidence.std_dev > threshold(sensor_id, '1 week')
ORDER BY confidence.std_dev DESC;
```

### Multi-Modal Search

```sql
-- Search across text and image embeddings
SELECT product.name, product.image_url,
       weighted_avg(
           cosine_similarity(text_embedding, :text_query),
           cosine_similarity(image_embedding, :image_query)
       ) AS combined_score
FROM products
WHERE combined_score > 0.75
ORDER BY combined_score DESC;
```
