use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::PathBuf;

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new(model_path: Option<PathBuf>) -> Result<Self> {
        let mut options = InitOptions::new(EmbeddingModel::AllMiniLML6V2Q);
        if let Some(path) = model_path {
            options = options.with_cache_dir(path);
        }
        let model = TextEmbedding::try_new(options)?;
        Ok(Self { model })
    }

    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.model.embed(vec![text], None)?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to generate embedding"))
    }
}
