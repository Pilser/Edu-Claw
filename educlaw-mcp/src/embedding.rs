// educlaw-mcp/src/embedding.rs
//
// HTTP client for the embedding server.
// Same protocol as memorymcp EmbeddingClient:
//   POST {url}/embed
//   Body: { "text": "...", "chunk_style": "recursive", "chunk_size": 100 }
//   Response: { "embedding": [f32; 384] }

use tracing::{debug, error, info};

/// Fallback embedding dimension used only in tests / when config is unavailable.
/// Always prefer `Config::embedding_dim` at runtime.
pub const DEFAULT_EMBEDDING_DIM: usize = 384;

#[derive(Clone)]
pub struct EmbeddingClient {
    api_url: String,
    client: reqwest::Client,
}

impl EmbeddingClient {
    pub fn new(api_url: String) -> Self {
        Self { api_url, client: reqwest::Client::new() }
    }

    /// Generate a `dim`-dimensional embedding for `text`.
    /// Pass `Config::embedding_dim` for `expected_dim`.
    pub async fn embed_with_dim(&self, text: &str, expected_dim: usize) -> anyhow::Result<Vec<f32>> {
        let v = self.embed(text).await?;
        if v.len() != expected_dim {
            anyhow::bail!(
                "embedding dimension mismatch: model returned {} but EMBEDDING_DIM={}",
                v.len(),
                expected_dim
            );
        }
        Ok(v)
    }

    /// Generate an embedding without dimension checking.
    /// Used by health_check and callers that validate dim elsewhere.
    pub async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        if text.is_empty() {
            anyhow::bail!("cannot embed empty text");
        }

        let body = serde_json::json!({
            "text": text,
            "chunk_style": "recursive",
            "chunk_size": 100
        });

        debug!("embedding {} chars", text.len());

        let resp: serde_json::Value = self
            .client
            .post(format!("{}/embed", self.api_url))
            .json(&body)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await?
            .json()
            .await?;

        let embedding: Vec<f32> = resp
            .get("embedding")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| anyhow::anyhow!("embedding server returned no 'embedding' field"))?;

        // Dimension is validated by the caller (SearchIndex) which knows
        // the expected dim from config. Here we just reject empty embeddings.
        if embedding.is_empty() {
            anyhow::bail!("embedding server returned empty vector");
        }

        // Sanity check: no NaN or Inf
        let bad = embedding.iter().filter(|&&x| !x.is_finite()).count();
        if bad > 0 {
            anyhow::bail!("embedding contains {bad} non-finite values");
        }

        Ok(embedding)
    }

    /// Verify the embedding server is reachable before starting.
    pub async fn health_check(&self) -> anyhow::Result<()> {
        debug!("health-checking embedding server: {}", self.api_url);
        self.embed("health check").await?;
        info!("embedding server OK at {}", self.api_url);
        Ok(())
    }
}
