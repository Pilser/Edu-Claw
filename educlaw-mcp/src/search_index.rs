// educlaw-mcp/src/search_index.rs
//
// LanceDB-powered hybrid search index.
//
// Storage: a LanceDB table "class_chunks" at EDUCLAW_SEARCH_DB (directory path).
//
// Schema:
//   class_id   TEXT   — class identifier (multi-tenant isolation)
//   file_path  TEXT   — source file path (relative to class root)
//   chunk_text TEXT   — raw text of the chunk (used for BM25 FTS)
//   embedding  FLOAT[dim] — dense embedding vector
//
// Indexes (rebuilt after each index_class_file call):
//   FTS on chunk_text  — BM25 inverted index via tantivy (lance-index)
//   IVF-HNSW-SQ on embedding — ANN vector index
//
// Search: vector KNN + BM25 full-text, merged with Reciprocal Rank Fusion.
// Class isolation: SQL predicate filter `class_id = 'X'` on every query.
// SQL injection: class_id is validated to [a-zA-Z0-9_-] before use in SQL.

use std::collections::HashSet;
use std::sync::Arc;

use arrow_array::{
    FixedSizeListArray, RecordBatch, StringArray,
    types::Float32Type,
};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lance_index::scalar::FullTextSearchQuery;
use lancedb::{
    connect,
    database::CreateTableMode,
    index::Index,
    query::{ExecutableQuery, QueryBase, Select},
};
use serde::Serialize;
use tracing::{debug, info};

// ── Result type ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub class_id: String,
    pub file_path: String,
    pub chunk_text: String,
    /// RRF relevance score — higher is more relevant (combines vector + BM25 ranks).
    pub score: f64,
}

// ── SearchIndex ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SearchIndex {
    tbl: lancedb::Table,
    /// Configured embedding dimension — callers use this to validate incoming vectors.
    pub dim: usize,
}

impl SearchIndex {
    /// Open (or create) the LanceDB table and return a ready index.
    /// `db_path` is a directory; LanceDB manages it.
    /// `dim` must match `Config::embedding_dim`.
    pub async fn open(db_path: &str, dim: usize) -> anyhow::Result<Self> {
        let conn = connect(db_path).execute().await?;
        let schema = chunk_schema(dim);

        // Create the table if it doesn't exist; open it if it does.
        let tbl = conn
            .create_empty_table("class_chunks", schema)
            .mode(CreateTableMode::exist_ok(|req| req))
            .execute()
            .await?;

        info!("search index ready (dim={dim}, path={db_path})");
        Ok(Self { tbl, dim })
    }

    // ── Write ─────────────────────────────────────────────────────────────

    /// Insert one chunk and its embedding as a single Arrow row.
    pub async fn insert_chunk(
        &self,
        class_id: String,
        file_path: String,
        chunk_text: String,
        embedding: Vec<f32>,
    ) -> anyhow::Result<()> {
        let schema = chunk_schema(self.dim);

        let class_ids   = StringArray::from(vec![class_id]);
        let file_paths  = StringArray::from(vec![file_path]);
        let chunk_texts = StringArray::from(vec![chunk_text]);
        let embeddings  = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
            std::iter::once(Some(embedding.into_iter().map(Some).collect::<Vec<_>>())),
            self.dim as i32,
        );

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(class_ids),
                Arc::new(file_paths),
                Arc::new(chunk_texts),
                Arc::new(embeddings),
            ],
        )?;

        self.tbl.add(batch).execute().await?;
        debug!("inserted chunk");
        Ok(())
    }

    /// Rebuild the BM25 FTS index and the ANN vector index.
    /// Call this once after all chunks for a file have been inserted.
    /// Skips ANN index when the table has fewer than 256 rows (LanceDB
    /// requires enough data to partition).
    pub async fn rebuild_fts_index(&self) -> anyhow::Result<()> {
        // FTS index (BM25) — always rebuild
        self.tbl
            .create_index(&["chunk_text"], Index::FTS(Default::default()))
            .replace(true)
            .execute()
            .await?;

        // ANN vector index — IVF-HNSW-SQ; skip if not enough rows
        let n = self.tbl.count_rows(None).await?;
        if n >= 256 {
            use lancedb::index::vector::IvfHnswSqIndexBuilder;
            self.tbl
                .create_index(&["embedding"], Index::IvfHnswSq(IvfHnswSqIndexBuilder::default()))
                .replace(true)
                .execute()
                .await?;
            info!("rebuilt FTS + IVF-HNSW-SQ indexes ({n} rows)");
        } else {
            info!("rebuilt FTS index ({n} rows; ANN index deferred until ≥256 rows)");
        }

        Ok(())
    }

    /// Delete all indexed chunks for a specific file (used before re-indexing).
    /// Returns the number of rows deleted.
    pub async fn delete_chunks_for_file(
        &self,
        class_id: &str,
        file_path: &str,
    ) -> anyhow::Result<usize> {
        sanitize_class_id(class_id)?;
        let safe_fp = file_path.replace('\'', "''");

        let before = self
            .tbl
            .count_rows(Some(format!(
                "class_id = '{class_id}' AND file_path = '{safe_fp}'"
            )))
            .await?;

        self.tbl
            .delete(&format!(
                "class_id = '{class_id}' AND file_path = '{safe_fp}'"
            ))
            .await?;

        Ok(before)
    }

    // ── Read ──────────────────────────────────────────────────────────────

    /// Hybrid search: vector KNN + BM25 FTS combined with RRF reranker.
    /// Results are filtered to `class_id` and ranked by relevance score (higher = better).
    pub async fn hybrid_search(
        &self,
        class_id: String,
        query_text: &str,
        query_embedding: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        sanitize_class_id(&class_id)?;
        let fts = FullTextSearchQuery::new(query_text.to_string());

        let batches: Vec<RecordBatch> = self
            .tbl
            .query()
            .full_text_search(fts)
            .only_if(format!("class_id = '{class_id}'"))
            .limit(limit)
            .nearest_to(query_embedding.as_slice())?
            .execute()
            .await?
            .try_collect()
            .await?;

        let mut results = Vec::new();
        for batch in &batches {
            use arrow_array::cast::AsArray;

            let class_ids   = batch.column_by_name("class_id").unwrap().as_string::<i32>();
            let file_paths  = batch.column_by_name("file_path").unwrap().as_string::<i32>();
            let chunk_texts = batch.column_by_name("chunk_text").unwrap().as_string::<i32>();

            // RRF reranker adds _relevance_score; fall back to 0.0 if absent
            let scores: Vec<f32> = batch
                .column_by_name("_relevance_score")
                .map(|c| c.as_primitive::<Float32Type>().values().to_vec())
                .unwrap_or_else(|| vec![0.0_f32; batch.num_rows()]);

            for i in 0..batch.num_rows() {
                results.push(SearchResult {
                    class_id:   class_ids.value(i).to_string(),
                    file_path:  file_paths.value(i).to_string(),
                    chunk_text: chunk_texts.value(i).to_string(),
                    score:      scores[i] as f64,
                });
            }
        }

        Ok(results)
    }

    /// Return all unique file paths indexed for a class.
    pub async fn indexed_files(&self, class_id: &str) -> anyhow::Result<Vec<String>> {
        sanitize_class_id(class_id)?;

        let batches: Vec<RecordBatch> = self
            .tbl
            .query()
            .only_if(format!("class_id = '{class_id}'"))
            .select(Select::columns(&["file_path"]))
            .execute()
            .await?
            .try_collect()
            .await?;

        let mut seen = HashSet::new();
        for batch in &batches {
            use arrow_array::cast::AsArray;
            let arr = batch.column_by_name("file_path").unwrap().as_string::<i32>();
            for i in 0..batch.num_rows() {
                seen.insert(arr.value(i).to_string());
            }
        }

        let mut paths: Vec<String> = seen.into_iter().collect();
        paths.sort();
        Ok(paths)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Build the Arrow schema for the class_chunks table.
fn chunk_schema(dim: usize) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("class_id",   DataType::Utf8, false),
        Field::new("file_path",  DataType::Utf8, false),
        Field::new("chunk_text", DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dim as i32,
            ),
            false,
        ),
    ]))
}

/// Validate a class_id or student_id: only alphanumerics, `-`, and `_` allowed.
/// This prevents SQL injection in LanceDB predicate strings.
fn sanitize_class_id(s: &str) -> anyhow::Result<()> {
    if s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        Ok(())
    } else {
        anyhow::bail!("id contains disallowed characters: {s:?}")
    }
}


// ── Schema builders ───────────────────────────────────────────────────────
