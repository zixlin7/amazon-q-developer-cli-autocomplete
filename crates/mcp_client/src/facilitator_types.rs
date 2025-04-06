use serde::{
    Deserialize,
    Serialize,
};
use thiserror::Error;

/// https://spec.modelcontextprotocol.io/specification/2024-11-05/server/utilities/pagination/#operations-supporting-pagination
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaginationSupportedOps {
    ResourcesList,
    ResourceTemplatesList,
    PromptsList,
    ToolsList,
}

impl PaginationSupportedOps {
    pub fn as_key(&self) -> &str {
        match self {
            PaginationSupportedOps::ResourcesList => "resources",
            PaginationSupportedOps::ResourceTemplatesList => "resourceTemplates",
            PaginationSupportedOps::PromptsList => "prompts",
            PaginationSupportedOps::ToolsList => "tools",
        }
    }
}

impl TryFrom<&str> for PaginationSupportedOps {
    type Error = OpsConversionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "resources/list" => Ok(PaginationSupportedOps::ResourcesList),
            "resources/templates/list" => Ok(PaginationSupportedOps::ResourceTemplatesList),
            "prompts/list" => Ok(PaginationSupportedOps::PromptsList),
            "tools/list" => Ok(PaginationSupportedOps::ToolsList),
            _ => Err(OpsConversionError::InvalidMethod),
        }
    }
}

#[derive(Error, Debug)]
pub enum OpsConversionError {
    #[error("Invalid method encountered")]
    InvalidMethod,
}

/// Result of listing resources operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesListResult {
    /// List of resources
    pub resources: Vec<serde_json::Value>,
    /// Optional cursor for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Result of listing resource templates operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplatesListResult {
    /// List of resource templates
    pub resource_templates: Vec<serde_json::Value>,
    /// Optional cursor for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Result of listing prompts operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptsListResult {
    /// List of prompts
    pub prompts: Vec<serde_json::Value>,
    /// Optional cursor for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Result of listing tools operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListResult {
    /// List of tools
    pub tools: Vec<serde_json::Value>,
    /// Optional cursor for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Vec<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MessageContent {
    /// Text content
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    #[serde(rename_all = "camelCase")]
    Image {
        /// base64-encoded-data
        data: String,
        mime_type: String,
    },
    /// Resource content
    Resource {
        /// The resource
        resource: Resource,
    },
}

/// Resource contents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ResourceContents {
    Text { text: String },
    Blob { data: Vec<u8> },
}

/// A resource in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Unique identifier for the resource
    pub uri: String,
    /// Human-readable title
    pub title: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Resource contents
    pub contents: ResourceContents,
}
