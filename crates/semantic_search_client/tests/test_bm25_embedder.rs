use std::path::Path;
use std::{
    env,
    fs,
};

use semantic_search_client::embedding::EmbeddingType;
use semantic_search_client::{
    ProgressStatus,
    SemanticSearchClient,
};

/// Test creating a client with BM25 embedder and performing basic operations
#[test]
fn test_bm25_client() {
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_bm25");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client with BM25 embedder
    let mut client = SemanticSearchClient::with_embedding_type(base_dir.clone(), EmbeddingType::BM25).unwrap();

    // Add a context from text
    let context_id = client
        .add_context_from_text(
            "BM25 is a keyword-based ranking function used in information retrieval",
            "BM25 Context",
            "Information about BM25 algorithm",
            true, // Make it persistent to have a proper name
        )
        .unwrap();

    // Verify the context was created
    let contexts = client.get_all_contexts();
    assert!(!contexts.is_empty());

    // Find the context by ID
    let context = contexts.iter().find(|c| c.id == context_id).unwrap();
    assert_eq!(context.name, "BM25 Context");

    // Test search with exact keyword match
    let results = client.search_context(&context_id, "keyword ranking", Some(5)).unwrap();

    // BM25 should find matches when there's keyword overlap
    assert!(!results.is_empty());

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

/// Test creating a client with BM25 embedder and adding a context from a file
#[test]
fn test_bm25_with_file() {
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_bm25_file");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a test file
    let test_file = temp_dir.join("bm25_test.txt");
    fs::write(&test_file, "BM25 is a bag-of-words retrieval function that ranks documents based on the query terms appearing in each document. It's commonly used in search engines and information retrieval systems.").unwrap();

    // Create a semantic search client with BM25 embedder
    let mut client = SemanticSearchClient::with_embedding_type(base_dir.clone(), EmbeddingType::BM25).unwrap();

    // Add a context from the file
    let context_id = client
        .add_context_from_path(
            Path::new(&test_file),
            "BM25 File Context",
            "Information about BM25 from a file",
            true, // Make it persistent to have a proper name
            None::<fn(ProgressStatus)>,
        )
        .unwrap();

    // Verify the context was created
    let contexts = client.get_all_contexts();
    assert!(!contexts.is_empty());

    // Find the context by ID
    let context = contexts.iter().find(|c| c.id == context_id).unwrap();
    assert_eq!(context.name, "BM25 File Context");

    // Test search with exact keyword match
    let results = client
        .search_context(&context_id, "search engines retrieval", Some(5))
        .unwrap();

    // BM25 should find matches when there's keyword overlap
    assert!(!results.is_empty());

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}

/// Test BM25 with persistent contexts
#[test]
fn test_bm25_persistent_context() {
    // Create a temporary directory for the test
    let temp_dir = env::temp_dir().join("semantic_search_test_bm25_persistent");
    let base_dir = temp_dir.join("semantic_search");
    fs::create_dir_all(&base_dir).unwrap();

    // Create a semantic search client with BM25 embedder
    let mut client = SemanticSearchClient::with_embedding_type(base_dir.clone(), EmbeddingType::BM25).unwrap();

    // Add a context and make it persistent
    let context_id = client
        .add_context_from_text(
            "BM25 is a keyword-based ranking function used in information retrieval",
            "BM25 Volatile",
            "Information about BM25 algorithm",
            false,
        )
        .unwrap();

    // Make it persistent
    client
        .make_persistent(&context_id, "BM25 Persistent", "A persistent BM25 context")
        .unwrap();

    // Create a new client to verify persistence
    let client2 = SemanticSearchClient::with_embedding_type(base_dir.clone(), EmbeddingType::BM25).unwrap();

    // Verify the context was persisted
    let contexts = client2.get_contexts();
    assert!(!contexts.is_empty());
    assert!(contexts.iter().any(|c| c.name == "BM25 Persistent"));

    // Clean up
    fs::remove_dir_all(temp_dir).unwrap_or(());
}
