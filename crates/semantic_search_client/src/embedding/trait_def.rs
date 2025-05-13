use crate::error::Result;

/// Embedding engine type to use
#[derive(Debug, Clone, Copy)]
pub enum EmbeddingType {
    /// Use Candle embedding engine
    Candle,
    /// Use ONNX embedding engine (only available on macOS and Windows)
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Onnx,
    /// Use Mock embedding engine (only available in tests)
    #[cfg(test)]
    Mock,
}

// We can't use #[derive(Default)] here because the default value depends on the target platform
#[cfg(any(target_os = "macos", target_os = "windows"))]
#[allow(clippy::derivable_impls)]
impl Default for EmbeddingType {
    fn default() -> Self {
        EmbeddingType::Onnx
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[allow(clippy::derivable_impls)]
impl Default for EmbeddingType {
    fn default() -> Self {
        EmbeddingType::Candle
    }
}

/// Common trait for text embedders
pub trait TextEmbedderTrait: Send + Sync {
    /// Generate an embedding for a text
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl TextEmbedderTrait for super::TextEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch(texts)
    }
}

impl TextEmbedderTrait for super::CandleTextEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch(texts)
    }
}

#[cfg(test)]
impl TextEmbedderTrait for super::MockTextEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(text)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch(texts)
    }
}
