mod trait_def;

#[cfg(test)]
mod benchmark_test;
mod benchmark_utils;
mod candle;
mod candle_models;
/// Mock embedder for testing
#[cfg(test)]
pub mod mock;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod onnx;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod onnx_models;

pub use benchmark_utils::{
    BenchmarkResults,
    BenchmarkableEmbedder,
    create_standard_test_data,
    run_standard_benchmark,
};
pub use candle::CandleTextEmbedder;
pub use candle_models::ModelType;
#[cfg(test)]
pub use mock::MockTextEmbedder;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub use onnx::TextEmbedder;
pub use trait_def::{
    EmbeddingType,
    TextEmbedderTrait,
};
