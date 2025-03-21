use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use convert_case::Casing;
use eyre::OptionExt;
use fig_api_client::model::{
    ToolResult,
    ToolResultContentBlock,
    ToolResultStatus,
};
use futures::{
    StreamExt,
    stream,
};
use serde::{
    Deserialize,
    Serialize,
};

use super::parser::ToolUse;
use super::tools::Tool;
use super::tools::custom_tool::{
    CustomToolClient,
    CustomToolConfig,
};
use super::tools::execute_bash::ExecuteBash;
use super::tools::fs_read::FsRead;
use super::tools::fs_write::FsWrite;
use super::tools::use_aws::UseAws;
use crate::cli::chat::tools::ToolSpec;
use crate::cli::chat::tools::custom_tool::CustomTool;

const NAMESPACE_DELIMITER: &str = "___";

// This is to mirror claude's config set up
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    mcp_servers: HashMap<String, CustomToolConfig>,
}

impl McpServerConfig {
    pub async fn load_config() -> eyre::Result<Self> {
        let config_path = fig_settings::settings::get_value("mcp.config")?
            .ok_or_eyre("No mcp config path specified")?
            .as_str()
            .ok_or_eyre("No valid path provided for mcp config")
            .map(|path| shellexpand::tilde(path))?
            .parse::<PathBuf>()?;
        let buf = tokio::fs::read(config_path).await?;
        Ok(serde_json::from_slice::<Self>(&buf)?)
    }
}

#[derive(Default)]
pub struct ToolManager {
    clients: HashMap<String, Arc<CustomToolClient>>,
}

impl ToolManager {
    pub async fn from_configs(config: McpServerConfig) -> Self {
        let McpServerConfig { mcp_servers } = config;
        let pre_initialized = mcp_servers
            .into_iter()
            .map(|(server_name, server_config)| {
                let server_name = server_name.to_case(convert_case::Case::Snake);
                let custom_tool_client = CustomToolClient::from_config(server_name.clone(), server_config);
                (server_name, custom_tool_client)
            })
            .collect::<Vec<(String, _)>>();
        let init_results = stream::iter(pre_initialized)
            .map(|(name, uninit_client)| async move { (name, uninit_client.await) })
            .buffer_unordered(10)
            .collect::<Vec<(String, _)>>()
            .await;
        let mut clients = HashMap::<String, Arc<CustomToolClient>>::new();
        for (name, init_res) in init_results {
            match init_res {
                Ok(client) => {
                    clients.insert(name, Arc::new(client));
                },
                Err(_e) => {
                    // TODO: log this
                },
            }
        }
        Self { clients }
    }

    pub async fn load_tools(&self) -> eyre::Result<HashMap<String, ToolSpec>> {
        let mut tool_specs = serde_json::from_str::<HashMap<String, ToolSpec>>(include_str!("tools/tool_index.json"))?;
        for client in self.clients.values() {
            match client.get_tool_spec().await {
                Ok((name, specs)) => {
                    // Each mcp server might have multiple tools.
                    // To avoid naming conflicts we are going to namespace it.
                    // This would also help us locate which mcp server to call the tool from.
                    for mut spec in specs {
                        spec.name = format!("{}{}{}", name, NAMESPACE_DELIMITER, spec.name);
                        tool_specs.insert(spec.name.clone(), spec);
                    }
                },
                Err(_e) => {
                    // TODO: log this. Perhaps also delete it from the list of tools we have?
                },
            }
        }
        Ok(tool_specs)
    }

    pub fn get_tool_from_tool_use(&self, value: ToolUse) -> Result<Tool, ToolResult> {
        let map_err = |parse_error| ToolResult {
            tool_use_id: value.id.clone(),
            content: vec![ToolResultContentBlock::Text(format!(
                "Failed to validate tool parameters: {parse_error}. The model has either suggested tool parameters which are incompatible with the existing tools, or has suggested one or more tool that does not exist in the list of known tools."
            ))],
            status: ToolResultStatus::Error,
        };

        Ok(match value.name.as_str() {
            "fs_read" => Tool::FsRead(serde_json::from_value::<FsRead>(value.args).map_err(map_err)?),
            "fs_write" => Tool::FsWrite(serde_json::from_value::<FsWrite>(value.args).map_err(map_err)?),
            "execute_bash" => Tool::ExecuteBash(serde_json::from_value::<ExecuteBash>(value.args).map_err(map_err)?),
            "use_aws" => Tool::UseAws(serde_json::from_value::<UseAws>(value.args).map_err(map_err)?),
            // Note that this name is namespaced with server_name{DELIMITER}tool_name
            name => {
                let (server_name, tool_name) = name.split_once(NAMESPACE_DELIMITER).ok_or(ToolResult {
                    tool_use_id: value.id.clone(),
                    content: vec![ToolResultContentBlock::Text(format!(
                        "The tool, \"{name}\" is supplied with incorrect name"
                    ))],
                    status: ToolResultStatus::Error,
                })?;
                let Some(client) = self.clients.get(server_name) else {
                    return Err(ToolResult {
                        tool_use_id: value.id,
                        content: vec![ToolResultContentBlock::Text(format!(
                            "The tool, \"{server_name}\" is not supported by the client"
                        ))],
                        status: ToolResultStatus::Error,
                    });
                };
                // The tool input schema has the shape of { type, properties }.
                // The field "params" expected by MCP is { name, arguments }, where name is the
                // name of the tool being invoked,
                // https://spec.modelcontextprotocol.io/specification/2024-11-05/server/tools/#calling-tools.
                // The field "arguments" is where ToolUse::args belong.
                let mut params = serde_json::Map::<String, serde_json::Value>::new();
                params.insert("name".to_owned(), serde_json::Value::String(tool_name.to_owned()));
                params.insert("arguments".to_owned(), value.args);
                let params = serde_json::Value::Object(params);
                let custom_tool = CustomTool {
                    name: tool_name.to_owned(),
                    client: client.clone(),
                    method: "tools/call".to_owned(),
                    params: Some(params),
                };
                Tool::Custom(custom_tool)
            },
        })
    }
}
