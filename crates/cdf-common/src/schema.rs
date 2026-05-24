//! Dynamic schema system for poly-modal tables.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A column definition in a CDF table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub dtype: DataType,
    pub nullable: bool,
    pub default: Option<serde_json::Value>,
    pub annotations: HashMap<String, String>,
}

impl ColumnDef {
    pub fn new(name: impl Into<String>, dtype: DataType) -> Self {
        Self {
            name: name.into(),
            dtype,
            nullable: true,
            default: None,
            annotations: HashMap::new(),
        }
    }

    pub fn required(mut self) -> Self {
        self.nullable = false;
        self
    }

    pub fn with_default(mut self, val: serde_json::Value) -> Self {
        self.default = Some(val);
        self
    }

    pub fn with_annotation(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.annotations.insert(key.into(), val.into());
        self
    }
}

/// Supported data types in CDF.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Bool,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
    String,
    Bytes,
    /// Embedding with known dimensionality.
    Vector { dimensions: usize, metric: String },
    /// Multi-dimensional tensor.
    Tensor { shape: Vec<u32>, dtype: TensorElementType },
    /// Probabilistic value.
    Distribution { family: String },
    /// Reference to external blob storage.
    BlobRef,
    /// Temporal bounds (auto-managed).
    Temporal,
    /// Graph edge reference.
    GraphEdge { edge_type: String },
    /// JSON-like unstructured data.
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorElementType {
    Float32,
    Float64,
    Int32,
    Int64,
}

/// Table schema with version for evolution tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub version: u64,
    pub columns: Vec<ColumnDef>,
    pub primary_key: Vec<String>,
    pub indexes: Vec<IndexDef>,
    pub options: HashMap<String, String>,
}

impl TableSchema {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: 1,
            columns: Vec::new(),
            primary_key: vec!["id".to_string()],
            indexes: Vec::new(),
            options: HashMap::new(),
        }
    }

    pub fn with_column(mut self, col: ColumnDef) -> Self {
        self.columns.push(col);
        self
    }

    pub fn with_primary_key(mut self, keys: Vec<String>) -> Self {
        self.primary_key = keys;
        self
    }

    pub fn with_index(mut self, index: IndexDef) -> Self {
        self.indexes.push(index);
        self
    }

    pub fn with_option(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.options.insert(key.into(), val.into());
        self
    }

    pub fn get_column(&self, name: &str) -> Option<&ColumnDef> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Validate a row against this schema.
    pub fn validate(&self, row: &HashMap<String, crate::PolyValue>) -> crate::Result<()> {
        for col in &self.columns {
            let has_value = row.contains_key(&col.name);
            if !col.nullable && !has_value {
                return Err(crate::CdfError::SchemaValidation {
                    field: col.name.clone(),
                    reason: "required field missing".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Produce a new schema version with an added column.
    pub fn evolve_add_column(&self, col: ColumnDef) -> Self {
        let mut next = self.clone();
        next.version += 1;
        next.columns.push(col);
        next
    }
}

/// Index definition for a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDef {
    pub name: String,
    pub index_type: IndexType,
    pub columns: Vec<String>,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexType {
    /// B-tree for range queries.
    BTree,
    /// Inverted index for text search.
    Inverted,
    /// HNSW for vector similarity.
    Hnsw,
    /// Adjacency index for graph traversal.
    Graph,
    /// Temporal index for time-slice queries.
    Temporal,
}

/// Schema registry trait.
pub trait SchemaRegistry: Send + Sync {
    fn register(&mut self, schema: TableSchema) -> crate::Result<()>;
    fn get(&self, name: &str) -> crate::Result<Option<TableSchema>>;
    fn get_version(&self, name: &str, version: u64) -> crate::Result<Option<TableSchema>>;
    fn list(&self) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_builder() {
        let schema = TableSchema::new("documents")
            .with_column(ColumnDef::new("id", DataType::String).required())
            .with_column(ColumnDef::new("title", DataType::String).required())
            .with_column(
                ColumnDef::new("embedding", DataType::Vector {
                    dimensions: 384,
                    metric: "cosine".to_string(),
                })
                .with_annotation("model", "all-MiniLM-L6-v2"),
            );

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.get_column("id").unwrap().dtype, DataType::String);
        assert!(!schema.get_column("id").unwrap().nullable);
    }

    #[test]
    fn test_validate_row() {
        let schema = TableSchema::new("test")
            .with_column(ColumnDef::new("name", DataType::String).required());

        let mut row = HashMap::new();
        row.insert("name".to_string(), crate::PolyValue::Text("Alice".to_string()));
        assert!(schema.validate(&row).is_ok());

        let empty = HashMap::new();
        assert!(schema.validate(&empty).is_err());
    }
}
